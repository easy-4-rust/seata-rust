use super::druid_db_type_parser_impl::strip_ascii_prefix;
use crate::{JavaObjectError, JavaString, MetadataError};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

/// 固定 Druid 1.2.28 的 JDBC 驱动类名推断。
/// 对应 Java: com.alibaba.druid.util.JdbcUtils#getDriverClassName。
/// 每个实例缓存类可用性探测结果，调用者提供自己的加载域。
#[derive(Debug, Default)]
pub struct DruidJdbcDriverResolver {
    mysql_modern: AtomicU8,
    clickhouse_modern: AtomicU8,
}

const MYSQL: &str = "com.mysql.cj.jdbc.Driver";
const CLICKHOUSE: &str = "com.clickhouse.jdbc.ClickHouseDriver";

// 顺序保留原分支优先级。mysql、clickhouse、db2 另外处理动态选择。
const DRIVERS: &[(&str, &str)] = &[
    ("jdbc:derby:", "org.apache.derby.jdbc.EmbeddedDriver"),
    ("jdbc:log4jdbc:", "net.sf.log4jdbc.DriverSpy"),
    ("jdbc:mariadb:", "org.mariadb.jdbc.Driver"),
    ("jdbc:tidb:", "io.tidb.bigdata.jdbc.TiDBDriver"),
    ("jdbc:oracle:", "oracle.jdbc.OracleDriver"),
    ("JDBC:oracle:", "oracle.jdbc.OracleDriver"),
    ("jdbc:alibaba:oracle:", "com.alibaba.jdbc.AlibabaDriver"),
    ("jdbc:oceanbase:", "com.alipay.oceanbase.jdbc.Driver"),
    (
        "jdbc:microsoft:",
        "com.microsoft.jdbc.sqlserver.SQLServerDriver",
    ),
    (
        "jdbc:sqlserver:",
        "com.microsoft.sqlserver.jdbc.SQLServerDriver",
    ),
    ("jdbc:sybase:Tds:", "com.sybase.jdbc2.jdbc.SybDriver"),
    ("jdbc:jtds:", "net.sourceforge.jtds.jdbc.Driver"),
    ("jdbc:fake:", "com.alibaba.druid.mock.MockDriver"),
    ("jdbc:mock:", "com.alibaba.druid.mock.MockDriver"),
    ("jdbc:postgresql:", "org.postgresql.Driver"),
    ("jdbc:edb:", "com.edb.Driver"),
    ("jdbc:odps:", "com.aliyun.odps.jdbc.OdpsDriver"),
    ("jdbc:hsqldb:", "org.hsqldb.jdbcDriver"),
    ("jdbc:sqlite:", "org.sqlite.JDBC"),
    ("jdbc:ingres:", "com.ingres.jdbc.IngresDriver"),
    ("jdbc:h2:", "org.h2.Driver"),
    ("jdbc:lealone:", "org.lealone.client.jdbc.JdbcDriver"),
    ("jdbc:mckoi:", "com.mckoi.JDBCDriver"),
    ("jdbc:cloudscape:", "COM.cloudscape.core.JDBCDriver"),
    ("jdbc:informix-sqli:", "com.informix.jdbc.IfxDriver"),
    ("jdbc:timesten:", "com.timesten.jdbc.TimesTenDriver"),
    ("jdbc:as400:", "com.ibm.as400.access.AS400JDBCDriver"),
    ("jdbc:sapdb:", "com.sap.dbtech.jdbc.DriverSapDB"),
    ("jdbc:JSQLConnect:", "com.jnetdirect.jsql.JSQLDriver"),
    ("jdbc:JTurbo:", "com.newatlanta.jturbo.driver.Driver"),
    ("jdbc:firebirdsql:", "org.firebirdsql.jdbc.FBDriver"),
    ("jdbc:interbase:", "interbase.interclient.Driver"),
    ("jdbc:pointbase:", "com.pointbase.jdbc.jdbcUniversalDriver"),
    ("jdbc:edbc:", "ca.edbc.jdbc.EdbcDriver"),
    ("jdbc:mimer:multi1:", "com.mimer.jdbc.Driver"),
    ("jdbc:dm:", "dm.jdbc.driver.DmDriver"),
    ("jdbc:kingbase:", "com.kingbase.Driver"),
    ("jdbc:kingbase8:", "com.kingbase8.Driver"),
    ("jdbc:gbase:", "com.gbase.jdbc.Driver"),
    ("jdbc:xugu:", "com.xugu.cloudjdbc.Driver"),
    ("jdbc:hive:", "org.apache.hive.jdbc.HiveDriver"),
    ("jdbc:hive2:", "org.apache.hive.jdbc.HiveDriver"),
    (
        "jdbc:phoenix:thin:",
        "org.apache.phoenix.queryserver.client.Driver",
    ),
    ("jdbc:phoenix://", "org.apache.phoenix.jdbc.PhoenixDriver"),
    ("jdbc:kylin:", "org.apache.kylin.jdbc.Driver"),
    (
        "jdbc:elastic:",
        "com.alibaba.xdriver.elastic.jdbc.ElasticDriver",
    ),
    ("jdbc:presto:", "com.facebook.presto.jdbc.PrestoDriver"),
    ("jdbc:trino:", "io.trino.jdbc.TrinoDriver"),
    ("jdbc:inspur:", "com.inspur.jdbc.KdDriver"),
    ("jdbc:polardb2:", "com.aliyun.polardb2.Driver"),
    ("jdbc:polardbx:", "com.alibaba.polardbx.Driver"),
    ("jdbc:polardb", "com.aliyun.polardb.Driver"),
    ("jdbc:highgo:", "com.highgo.jdbc.Driver"),
    ("jdbc:oscar", "com.oscar.Driver"),
    ("jdbc:dbcp:", "com.dbcp.jdbc.Driver"),
    ("jdbc:opengauss:", "org.opengauss.Driver"),
    ("jdbc:TAOS:", "com.taosdata.jdbc.TSDBDriver"),
    ("jdbc:TAOS-RS:", "com.taosdata.jdbc.rs.RestfulDriver"),
    ("jdbc:gbasedbt-sqli:", "com.gbasedbt.jdbc.Driver"),
    ("jdbc:sundb:", "csii.sundb.jdbc.SundbDriver"),
    ("jdbc:gaussdb:", "com.huawei.gaussdb.jdbc.Driver"),
];

impl DruidJdbcDriverResolver {
    /// 根据 jdbc_url 返回原驱动类名。动态分支尚无缓存时调用 class_available。
    /// 对应 Java: JdbcUtils#getDriverClassName；未知 URL 保留 SQLException 和原 UTF-16 消息。
    pub fn get_driver_class_name(
        &self,
        jdbc_url: Option<&JavaString>,
        class_available: impl FnOnce(&str) -> bool,
    ) -> Result<Option<JavaString>, MetadataError> {
        self.try_get_driver_class_name(jdbc_url, |name| Ok(class_available(name)))
    }

    /// 允许 class_available 返回宿主异常。探测失败直接返回该异常，保留缓存的原状态。
    /// 对应 Java: JdbcUtils#getDriverClassName 的动态驱动类探测。
    pub fn try_get_driver_class_name(
        &self,
        jdbc_url: Option<&JavaString>,
        class_available: impl FnOnce(&str) -> Result<bool, MetadataError>,
    ) -> Result<Option<JavaString>, MetadataError> {
        let Some(url) = jdbc_url else {
            return Ok(None);
        };
        let driver = if strip_ascii_prefix(url, "jdbc:mysql:").is_some() {
            probe_driver(
                &self.mysql_modern,
                MYSQL,
                "com.mysql.jdbc.Driver",
                class_available,
            )?
        } else if strip_ascii_prefix(url, "jdbc:clickhouse:").is_some() {
            probe_driver(
                &self.clickhouse_modern,
                CLICKHOUSE,
                "ru.yandex.clickhouse.ClickHouseDriver",
                class_available,
            )?
        } else if let Some(suffix) = strip_ascii_prefix(url, "jdbc:db2:") {
            // JDK 的 indexOf(':') 必须大于 0，开头的冒号不触发 Type 3。
            if suffix.starts_with(&[u16::from(b'/'), u16::from(b'/')]) {
                "com.ibm.db2.jcc.DB2Driver"
            } else if suffix
                .iter()
                .position(|&unit| unit == u16::from(b':'))
                .is_some_and(|index| index > 0)
            {
                "COM.ibm.db2.jdbc.net.DB2Driver"
            } else {
                "COM.ibm.db2.jdbc.app.DB2Driver"
            }
        } else {
            DRIVERS
                .iter()
                .find_map(|(prefix, name)| strip_ascii_prefix(url, prefix).map(|_| *name))
                .ok_or_else(|| unknown_driver(url))?
        };
        Ok(Some(JavaString::from(driver)))
    }
}

fn probe_driver(
    cache: &AtomicU8,
    modern: &'static str,
    legacy: &'static str,
    class_available: impl FnOnce(&str) -> Result<bool, MetadataError>,
) -> Result<&'static str, MetadataError> {
    const UNKNOWN: u8 = 0;
    const LEGACY: u8 = 1;
    const MODERN: u8 = 2;
    let selected = match cache.load(Ordering::Relaxed) {
        UNKNOWN => {
            // 探测期间允许同线程重入，外层探测完成后覆盖内层结果。
            let observed = if class_available(modern)? {
                MODERN
            } else {
                LEGACY
            };
            cache.store(observed, Ordering::Relaxed);
            observed
        }
        cached => cached,
    };
    Ok(if selected == MODERN { modern } else { legacy })
}

fn unknown_driver(url: &JavaString) -> MetadataError {
    let mut message: Vec<_> = "unknown jdbc driver : ".encode_utf16().collect();
    message.extend_from_slice(url.as_utf16());
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new(
            "java.sql.SQLException",
            Some(JavaString::from_utf16(message)),
        )
        .with_superclasses(["java.lang.Exception", "java.lang.Throwable"]),
    ))
}
