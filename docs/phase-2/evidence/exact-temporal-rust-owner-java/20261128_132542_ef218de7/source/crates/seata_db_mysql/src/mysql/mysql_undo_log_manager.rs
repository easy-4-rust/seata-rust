use super::undo_fragment_layout::UndoFragmentLayout;
use crate::{cache::MySqlTableMetaReader, undo::UndoSerializer, DbError};
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlRow},
    Connection, MySql, QueryBuilder, Row, Transaction,
};

/// 原Undo表的Normal写入、分片读取及删除；用同身份辅助连接核对当前结构，不提交业务或执行行补偿。
/// 对应 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager（部分实现）。
pub(crate) struct MySqlUndoLogManager;
impl MySqlUndoLogManager {
    /// 在原根身份锁内检查缺根或围栏分支；发现关联子片则拒绝空回滚。
    pub(crate) async fn require_no_rollback_children(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
    ) -> Result<(), DbError> {
        // 与 Normal 成员核对使用同一父号匹配。当前读取能看到晚于旧字典快照提交的子片。
        let child = sqlx::query(
            "SELECT branch_id FROM undo_log WHERE xid=? AND context=? LIMIT 1 FOR UPDATE",
        )
        .bind(xid)
        .bind(format!("branchId={branch_id}"))
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| DbError::driver("Undo children without Normal root", error))?;
        if child.is_some() {
            return Err(Self::unsupported(
                "Undo orphan children without Normal root",
            ));
        }
        Ok(())
    }

    /// 参数为持有提交授权恢复行锁的控制事务、原连接配置、schema及原身份；只清理原root／子片。
    /// 对应 Java: org.apache.seata.rm.datasource.undo.AbstractUndoLogManager#deleteUndoLog；避免批量笛卡尔积跨分支删除。
    pub(crate) async fn delete_committed(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
        xid: &str,
        branch_id: i64,
    ) -> Result<(), DbError> {
        Self::validate_current_table(tx, options, database).await?;
        let root = sqlx::query(
            "SELECT xid,branch_id,context FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE",
        )
        .bind(xid)
        .bind(branch_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| DbError::driver("commit Undo root identity", e))?;
        if let Some(row) = root {
            let context = Self::text(&row, "context")?;
            if Self::text(&row, "xid")?.as_bytes() != xid.as_bytes()
                || row
                    .try_get::<i64, _>("branch_id")
                    .map_err(|e| DbError::driver("commit Undo root branch", e))?
                    != branch_id
                || context
                    .get(..9)
                    .is_some_and(|v| v.eq_ignore_ascii_case("branchId="))
            {
                return Err(Self::unsupported(
                    "commit Undo exact original root ownership",
                ));
            }
            let removed = sqlx::query("DELETE FROM undo_log WHERE xid=? AND branch_id=?")
                .bind(xid)
                .bind(branch_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| DbError::driver("commit Undo root delete", e))?
                .rows_affected();
            if removed != 1 {
                return Err(Self::unsupported("commit Undo root row count"));
            }
        }
        // 不读取任意大正文；每批当前读取锁定64个原主键，全部清理仍在同一个外层事务内。
        let expected = format!("branchId={branch_id}");
        loop {
            let rows=sqlx::query("SELECT xid,branch_id,context FROM undo_log WHERE xid=? AND context=? ORDER BY branch_id LIMIT 64 FOR UPDATE")
                .bind(xid).bind(&expected).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("commit Undo child identities",e))?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                if Self::text(&row, "xid")?.as_bytes() != xid.as_bytes()
                    || Self::text(&row, "context")?.as_bytes() != expected.as_bytes()
                {
                    return Err(Self::unsupported(
                        "commit Undo exact original child ownership",
                    ));
                }
                let child: i64 = row
                    .try_get("branch_id")
                    .map_err(|e| DbError::driver("commit Undo child branch", e))?;
                let removed =
                    sqlx::query("DELETE FROM undo_log WHERE xid=? AND branch_id=? AND context=?")
                        .bind(xid)
                        .bind(child)
                        .bind(&expected)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| DbError::driver("commit Undo child delete", e))?
                        .rows_affected();
                if removed != 1 {
                    return Err(Self::unsupported("commit Undo child row count"));
                }
            }
        }
        Ok(())
    }
    /// 参数为持有root锁的原事务/身份；返回有界重组字节及精确子号，全部身份/状态先验证后发布。
    /// 对应 Java: MySQLUndoLogManager#getSubRollbackInfo与AbstractUndoLogManager#getRollbackInfo。
    pub(crate) async fn read_normal(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
    ) -> Result<(Vec<u8>, Vec<i64>), DbError> {
        Self::read_normal_for_serializer(tx, xid, branch_id, UndoSerializer::Jackson).await
    }

    /// 使用控制事务固定的解析器校验根行格式，再读取原分片及身份。
    /// 对应 Java: MySQLUndoLogManager#getSubRollbackInfo。
    pub(crate) async fn read_normal_for_serializer(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
        serializer: UndoSerializer,
    ) -> Result<(Vec<u8>, Vec<i64>), DbError> {
        let row = Self::normal_header(tx, xid, branch_id).await?;
        let text = Self::text(&row, "context")?;
        let mut context = std::collections::BTreeMap::new();
        for part in text.split('&') {
            let (key, value) = part
                .split_once('=')
                .ok_or(Self::unsupported("Undo context entry"))?;
            if !matches!(key, "serializer" | "compressorType" | "map" | "subId")
                || context.insert(key, value).is_some()
            {
                return Err(Self::unsupported("Undo context duplicate/unknown key"));
            }
        }
        serializer.require_context(
            context.get("serializer").copied(),
            context.get("compressorType").copied(),
        )?;
        if let Some(packet) = context.get("map") {
            if !packet
                .parse::<u64>()
                .is_ok_and(|v| (1024..=1_073_741_824).contains(&v))
            {
                return Err(Self::unsupported("Undo recorded packet size"));
            }
        }
        let mut ids = Vec::new();
        // Java 的 StringUtils.isBlank 使用固定 JDK 的 Character.isWhitespace(char)。
        if let Some(value) = context.get("subId").filter(|value| {
            !value.encode_utf16().all(|unit| {
                matches!(unit, 0x0009..=0x000d | 0x001c..=0x0020 | 0x1680
                    | 0x2000..=0x2006 | 0x2008..=0x200a | 0x2028 | 0x2029 | 0x205f | 0x3000)
            })
        }) {
            for value in value.strip_suffix(',').unwrap_or(value).split(',') {
                let id = value
                    .parse::<i64>()
                    .map_err(|source| DbError::InvalidUndoSubId { source })?;
                if id <= 0
                    || id == branch_id
                    || id.to_string() != value
                    || ids.contains(&id)
                    || ids.len() >= 64
                {
                    return Err(Self::unsupported("Undo duplicate/invalid subId"));
                }
                ids.push(id);
            }
        }
        let root_length = Self::normal_length(&row)?;
        let mut total = root_length;
        let mut child_lengths = Vec::with_capacity(ids.len());
        let child_context = format!("branchId={branch_id}");
        for id in &ids {
            let header = Self::normal_header(tx, xid, *id).await?;
            if Self::text(&header, "context")? != child_context {
                return Err(Self::unsupported("Undo original child context"));
            }
            let length = Self::normal_length(&header)?;
            total = total
                .checked_add(length)
                .ok_or(Self::unsupported("Undo aggregate bytes overflow"))?;
            if total > UndoFragmentLayout::MAX_BYTES {
                return Err(Self::unsupported("Undo aggregate byte budget"));
            }
            child_lengths.push((*id, length));
        }
        // 原父号下的孤儿片同样不能被悄悄遗留或冒充其他分支的片。
        let actual: Vec<i64> = sqlx::query_scalar(
            "SELECT branch_id FROM undo_log WHERE xid=? AND context=? LIMIT 65 FOR UPDATE",
        )
        .bind(xid)
        .bind(&child_context)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::driver("Undo child membership", e))?;
        // 根行已保证预期 ID 唯一；等长且每个预期 ID 都出现，才能排除重复行掩盖缺片。
        if actual.len() != ids.len() || ids.iter().any(|id| !actual.contains(id)) {
            return Err(Self::unsupported("Undo orphan/missing child membership"));
        }
        // 两个正文响应都读到结束，避免调用方在未完成的协议流上执行下一步。
        let mut root: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE",
        )
        .bind(xid)
        .bind(branch_id)
        .persistent(false)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::driver("Undo current body", e))?;
        let mut bytes = match root.as_mut_slice() {
            [part] if part.len() == root_length => std::mem::take(part),
            [_] => return Err(Self::unsupported("Undo locked body length changed")),
            [] => {
                return Err(DbError::driver(
                    "Undo current body",
                    sqlx::Error::RowNotFound,
                ))
            }
            _ => return Err(Self::unsupported("Undo exact locked body row count")),
        };
        if !child_lengths.is_empty() {
            // 原唯一键和已锁定的头部给出最多 64 行及全部长度上限，批量读取仍使用原 XID。
            let mut query = QueryBuilder::<MySql>::new(
                "SELECT branch_id,rollback_info FROM undo_log WHERE branch_id IN (",
            );
            {
                let mut parameters = query.separated(", ");
                for id in &ids {
                    parameters.push_bind(*id);
                }
            }
            query.push(") AND xid=").push_bind(xid).push(" FOR UPDATE");
            let rows = query
                .build_query_as::<(i64, Vec<u8>)>()
                .persistent(false)
                .fetch_all(&mut **tx)
                .await
                .map_err(|e| DbError::driver("Undo child body", e))?;
            if rows.len() != child_lengths.len() {
                return Err(Self::unsupported("Undo exact child body row count"));
            }
            let mut by_id: std::collections::BTreeMap<_, _> = rows.into_iter().collect();
            bytes.reserve_exact(total - root_length);
            for (id, length) in child_lengths {
                let part = by_id
                    .remove(&id)
                    .ok_or(Self::unsupported("Undo exact child body membership"))?;
                if part.len() != length {
                    return Err(Self::unsupported("Undo locked body length changed"));
                }
                bytes.extend_from_slice(&part);
            }
        }
        Ok((bytes, ids))
    }
    async fn normal_header(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
    ) -> Result<MySqlRow, DbError> {
        let row=sqlx::query("SELECT xid,branch_id,context,log_status,CAST(OCTET_LENGTH(rollback_info) AS SIGNED) AS bytes FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE").bind(xid).bind(branch_id).fetch_optional(&mut **tx).await.map_err(|e|DbError::driver("Undo normal header",e))?.ok_or(DbError::RecoveryConflict)?;
        if Self::text(&row, "xid")? != xid
            || row
                .try_get::<i64, _>("branch_id")
                .map_err(|e| DbError::driver("Undo original row identity", e))?
                != branch_id
        {
            return Err(Self::unsupported("Undo exact original row identity"));
        }
        let status: i32 = row
            .try_get("log_status")
            .map_err(|e| DbError::driver("Undo normal state", e))?;
        if status != 0 {
            return Err(Self::unsupported("Undo original Normal state"));
        }
        Self::normal_length(&row)?;
        Ok(row)
    }
    fn normal_length(row: &MySqlRow) -> Result<usize, DbError> {
        let length: i64 = row
            .try_get("bytes")
            .map_err(|e| DbError::driver("Undo body length", e))?;
        if !(1..=UndoFragmentLayout::MAX_BYTES as i64).contains(&length) {
            return Err(Self::unsupported("Undo body byte budget"));
        }
        Ok(length as usize)
    }
    /// 参数为已锁定/核对的原身份与子号；返回全部删除成功，不隐式提交或删除其他分支。
    /// 对应 Java: AbstractUndoLogManager#deleteUndoLog的原root/子行删除范围。
    pub(crate) async fn delete_normal(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
        ids: &[i64],
    ) -> Result<(), DbError> {
        let child_context = format!("branchId={branch_id}");
        for id in ids.iter().copied().chain(std::iter::once(branch_id)) {
            // 保留唯一键查找，并在实际删除时再次核对原字节身份及子片父号。
            let (sql, parent) = if id == branch_id {
                (
                    "DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0 AND CAST(xid AS BINARY)=CAST(? AS BINARY)",
                    None,
                )
            } else {
                (
                    "DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0 AND CAST(xid AS BINARY)=CAST(? AS BINARY) AND CAST(context AS BINARY)=CAST(? AS BINARY)",
                    Some(child_context.as_str()),
                )
            };
            let mut delete = sqlx::query(sql).bind(xid).bind(id).bind(xid);
            if let Some(parent) = parent {
                delete = delete.bind(parent);
            }
            let removed = delete
                .execute(&mut **tx)
                .await
                .map_err(|e| DbError::driver("Undo normal delete", e))?;
            if removed.rows_affected() != 1 {
                return Err(Self::unsupported("Undo exact deleted row count"));
            }
        }
        Ok(())
    }
    /// 参数为独立控制事务/原连接配置/schema/原身份；返回尚未提交的决定，不承诺业务历史或二阶段完成。
    pub(crate) async fn prepare_fence(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
        xid: &str,
        branch_id: i64,
    ) -> Result<super::UndoFenceDecision, DbError> {
        Self::validate_current_table(tx, options, database).await?;
        let flush: i64 =
            sqlx::query_scalar("SELECT CAST(@@GLOBAL.innodb_flush_log_at_trx_commit AS SIGNED)")
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| DbError::driver("Undo fence durability", e))?;
        if flush != 1 {
            return Err(Self::unsupported(
                "Undo fence requires durable InnoDB commit",
            ));
        }
        // 锁定原唯一身份；先读长度，畸形guard不能触发任意大BLOB分配。
        let row = sqlx::query("SELECT xid,branch_id,log_status,context,CAST(OCTET_LENGTH(rollback_info) AS SIGNED) AS bytes FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE")
            .bind(xid).bind(branch_id).fetch_optional(&mut **tx).await.map_err(|e| DbError::driver("Undo fence identity lock", e))?;
        if let Some(row) = row {
            // 原表可能使用不区分大小写/重音的排序规则；SQL 命中不等于原身份逐字节一致。
            if Self::text(&row, "xid")?.as_bytes() != xid.as_bytes()
                || row
                    .try_get::<i64, _>("branch_id")
                    .map_err(|e| DbError::driver("Undo fence original branch", e))?
                    != branch_id
            {
                return Err(Self::unsupported("Undo fence exact original identity"));
            }
            let status: i32 = row
                .try_get("log_status")
                .map_err(|e| DbError::driver("Undo fence status", e))?;
            if status == 0 {
                return Ok(super::UndoFenceDecision::NormalRow);
            }
            let context = Self::text(&row, "context")?;
            let bytes: i64 = row
                .try_get("bytes")
                .map_err(|e| DbError::driver("Undo fence size", e))?;
            if status != 1
                || bytes != 2
                || !matches!(
                    context.as_str(),
                    "serializer=jackson&compressorType=NONE"
                        | "compressorType=NONE&serializer=jackson"
                )
            {
                return Err(Self::unsupported("Undo fence existing guard shape"));
            }
            // 前次FOR UPDATE读到的记录可能晚于本事务RR快照，内容也必须使用当前读取。
            let content: Vec<u8> = sqlx::query_scalar(
                "SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=? FOR UPDATE",
            )
            .bind(xid)
            .bind(branch_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo fence guard content", e))?;
            if content != b"{}" {
                return Err(Self::unsupported("Undo fence existing guard content"));
            }
            return Ok(super::UndoFenceDecision::ExistingGuard);
        }
        Self::insert_guard(tx, xid, branch_id).await?;
        Ok(super::UndoFenceDecision::InsertedGuard)
    }

    /// 在已校验的控制事务内为原唯一身份写入待提交的 GlobalFinished 围栏。
    /// 调用方须已锁定原身份并确认无日志或完成补偿删除；唯一冲突及死锁不重试。
    pub(crate) async fn insert_guard(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
    ) -> Result<(), DbError> {
        let inserted = sqlx::query("INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))")
            .bind(branch_id).bind(xid).bind("serializer=jackson&compressorType=NONE").bind(b"{}".as_slice())
            .execute(&mut **tx).await.map_err(|e| DbError::driver("Undo fence insert", e))?;
        if inserted.rows_affected() != 1 {
            return Err(Self::unsupported("Undo fence inserted row count"));
        }
        Ok(())
    }
    /// 参数为原事务/连接配置/schema/身份与已验证原格式字节；成功表示同事务写入全部片，错误不重试。
    pub(crate) async fn flush(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
        xid: &str,
        branch_id: i64,
        bytes: &[u8],
    ) -> Result<(), DbError> {
        Self::validate_current_table(tx, options, database).await?;
        let packet: u64 = sqlx::query_scalar("SELECT @@session.max_allowed_packet")
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo packet limit", e))?;
        if !(1024..=1_073_741_824).contains(&packet) {
            return Err(Self::unsupported("Undo max_allowed_packet"));
        }
        let mut layout = UndoFragmentLayout::new(branch_id, packet, bytes)?;
        for _ in 0..layout.child_count() {
            // 以数据库本机生成的身份关联子片；冲突失败，不重发或覆盖。
            let raw: u64 = sqlx::query_scalar("SELECT UUID_SHORT()")
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| DbError::driver("Undo subId allocation", e))?;
            let id = i64::try_from(raw)
                .map_err(|_| Self::unsupported("Undo subId outside positive long"))?;
            layout.add_child(id)?;
        }
        Self::write_fragments(tx, xid, &layout).await
    }

    /// 参数为原事务、XID 和完整布局；按布局插入全部片，事务由调用方完成。
    pub(super) async fn write_fragments(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        layout: &UndoFragmentLayout<'_>,
    ) -> Result<(), DbError> {
        for (id, context, chunk) in layout.rows()? {
            Self::insert(tx, xid, id, context, chunk).await?;
        }
        Ok(())
    }

    async fn insert(
        tx: &mut Transaction<'static, MySql>,
        xid: &str,
        branch_id: i64,
        context: &str,
        bytes: &[u8],
    ) -> Result<(), DbError> {
        // 对齐原 Java 的逐行语句作用域，INSERT 执行后由 SQLx 释放其服务端句柄。
        let result = sqlx::query("INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,0,now(6),now(6))")
            .persistent(false)
            .bind(branch_id)
            .bind(xid)
            .bind(context)
            .bind(bytes)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo insert", e))?;
        if result.rows_affected() != 1 {
            return Err(Self::unsupported("Undo inserted row count"));
        }
        Ok(())
    }
    fn unsupported(feature: &'static str) -> DbError {
        DbError::Unsupported { feature }
    }
    fn text(row: &MySqlRow, name: &str) -> Result<String, DbError> {
        row.try_get(name)
            .map_err(|e| DbError::driver("Undo table metadata", e))
    }
    /// 参数为原事务与固定 schema；在同一 MDL 下核对原七列日志结构，不修改表。
    pub(crate) async fn validate_table(
        tx: &mut Transaction<'_, MySql>,
        database: &str,
    ) -> Result<(), DbError> {
        // 固定表名/绑定schema，无DDL；空结果锁定读取使后续结构校验和插入都持有同一MDL。
        sqlx::query("SELECT branch_id FROM undo_log WHERE 1=0 FOR UPDATE")
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo metadata lock", e))?;
        let (has_trigger, has_foreign_key) =
            crate::current_table_constraints::CurrentTableConstraints::read(
                tx, database, "undo_log",
            )
            .await?;
        let row=sqlx::query("SELECT CAST(ENGINE AS CHAR CHARACTER SET utf8mb4) AS engine,CAST(TABLE_TYPE AS CHAR CHARACTER SET utf8mb4) AS kind FROM information_schema.TABLES WHERE TABLE_SCHEMA=? AND TABLE_NAME='undo_log'")
            .bind(database).fetch_one(&mut **tx).await.map_err(|e|DbError::driver("Undo table",e))?;
        if Self::text(&row, "engine")? != "InnoDB" || Self::text(&row, "kind")? != "BASE TABLE" {
            return Err(Self::unsupported("Undo transactional base table"));
        }
        if has_trigger || has_foreign_key {
            return Err(Self::unsupported("Undo trigger or foreign key"));
        }
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4) AS name,CAST(DATA_TYPE AS CHAR CHARACTER SET utf8mb4) AS kind,CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4) AS full_type,CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4) AS nullable,CAST(EXTRA AS CHAR CHARACTER SET utf8mb4) AS extra,CAST(CHARACTER_SET_NAME AS CHAR CHARACTER SET utf8mb4) AS charset,CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS chars,CAST(DATETIME_PRECISION AS SIGNED) AS precision_value FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME='undo_log' ORDER BY ORDINAL_POSITION LIMIT 8")
            .bind(database).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("Undo columns",e))?;
        let expected = [
            ("branch_id", "bigint"),
            ("xid", "varchar"),
            ("context", "varchar"),
            ("rollback_info", "longblob"),
            ("log_status", "int"),
            ("log_created", "datetime"),
            ("log_modified", "datetime"),
        ];
        if columns.len() != expected.len() {
            return Err(Self::unsupported("Undo seven-column shape"));
        }
        for (row, (name, kind)) in columns.iter().zip(expected) {
            if Self::text(row, "name")? != name
                || Self::text(row, "kind")? != kind
                || Self::text(row, "nullable")? != "NO"
                || !Self::text(row, "extra")?.is_empty()
                || Self::text(row, "full_type")?.contains("unsigned")
            {
                return Err(Self::unsupported("Undo original column shape"));
            }
            if kind == "varchar"
                && (row
                    .try_get::<Option<i64>, _>("chars")
                    .map_err(|e| DbError::driver("Undo column width", e))?
                    != Some(128)
                    || row
                        .try_get::<Option<String>, _>("charset")
                        .map_err(|e| DbError::driver("Undo column charset", e))?
                        .as_deref()
                        != Some("utf8mb4"))
            {
                return Err(Self::unsupported("Undo text capacity or charset"));
            }
            if kind == "datetime"
                && row
                    .try_get::<Option<i64>, _>("precision_value")
                    .map_err(|e| DbError::driver("Undo timestamp precision", e))?
                    != Some(6)
            {
                return Err(Self::unsupported("Undo microsecond timestamps"));
            }
        }
        let indexes:i64=sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT INDEX_NAME FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME='undo_log' AND NON_UNIQUE=0 GROUP BY INDEX_NAME HAVING COUNT(*)=2 AND GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX)='xid,branch_id' AND COUNT(SUB_PART)=0 AND SUM(IS_VISIBLE='YES')=2) AS original_identity")
            .bind(database).fetch_one(&mut **tx).await.map_err(|e|DbError::driver("Undo unique identity",e))?;
        if indexes != 1 {
            return Err(Self::unsupported("Undo original unique identity"));
        }
        // 业务表可能先建立旧字典快照；不能将旧 InnoDB 字段当作当前物理引擎证明。
        MySqlTableMetaReader::verify_current_engine(tx, "undo_log").await?;
        Ok(())
    }

    /// 参数为持有原 Undo MDL 的事务、原连接配置与 schema；返回当前既有表结构规范的校验结果。
    /// 先保留原事务错误优先级，再以同身份、同实例的新事务核对字典快照，所有操作受调用方原期限覆盖。
    async fn validate_current_table(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
    ) -> Result<(), DbError> {
        Self::validate_table(tx, database).await?;
        let mut connection =
            crate::current_table_constraints::CurrentTableConstraints::connect_current(
                tx, options, database, "undo_log",
            )
            .await?;
        let mut current = connection
            .begin()
            .await
            .map_err(|error| DbError::driver("current Undo metadata begin", error))?;
        Self::validate_table(&mut current, database).await?;
        current
            .rollback()
            .await
            .map_err(|error| DbError::driver("current Undo metadata rollback", error))?;
        connection
            .close()
            .await
            .map_err(|error| DbError::driver("current Undo metadata close", error))?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::MySqlUndoLogManager;
    use crate::mysql::UndoFenceDecision;
    use crate::{MySqlDataSource, MySqlOptions, UndoFenceObservation};
    #[tokio::test]
    #[ignore = "requires owned real MySQL fixture"]
    async fn older_control_snapshot_reads_the_later_committed_guard() {
        let options = MySqlOptions::new(
            "127.0.0.1".into(),
            std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
            "root".into(),
            std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
            std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
        )
        .unwrap()
        .with_insecure_loopback_tls()
        .unwrap();
        let ds = MySqlDataSource::connect(options.clone()).await.unwrap();
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(1)
            .connect_with(options.connect_options())
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("SET SESSION transaction_isolation='REPEATABLE-READ'")
            .execute(&mut *connection)
            .await
            .unwrap();
        connection.close_on_drop();
        let mut tx = sqlx::Transaction::begin(connection, None).await.unwrap();
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM undo_log")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(before, 0);
        assert_eq!(
            ds.fence_undo_log("fence:snapshot", 43).await.unwrap(),
            UndoFenceObservation::Fenced { inserted: true }
        );
        let still_old: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM undo_log")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            still_old, 0,
            "fixture must really retain an older RR snapshot"
        );
        let decision = MySqlUndoLogManager::prepare_fence(
            &mut tx,
            &ds.control_pool().connect_options(),
            &std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
            "fence:snapshot",
            43,
        )
        .await
        .expect("existing guard must use a current read");
        assert!(matches!(decision, UndoFenceDecision::ExistingGuard));
        tx.rollback().await.unwrap();
        ds.close().await.unwrap();
        pool.close().await;
    }
}
