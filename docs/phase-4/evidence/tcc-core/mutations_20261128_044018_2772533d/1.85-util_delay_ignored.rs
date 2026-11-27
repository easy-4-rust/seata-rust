use super::action_context_util::ActionContextUtil;
use crate::{BusinessActionContext, TccError, TccTransaction};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

/// 合并当前业务参数并按 delayReport 自动报告；显式借用上下文，避免异步线程切换串用身份。
/// 对应 Java: org.apache.seata.rm.tcc.api.BusinessActionContextUtil。
/// 不保存全局 ThreadLocal，也不提供裸身份报告；报告权限只来自当前受控事务。
#[derive(Debug)]
pub struct BusinessActionContextUtil;
impl BusinessActionContextUtil {
    /// 参数为当前上下文、事务、业务键和可选原 JSON 值；null/缺失/同值/延迟返回 false。
    /// 真正变化且不延迟时等待一次正式报告，返回实际报告结果；错误不清除变更也不重试。
    pub async fn add_context(
        context: &mut BusinessActionContext,
        transaction: &mut dyn TccTransaction,
        key: &str,
        value: Option<&RawValue>,
    ) -> Result<bool, TccError> {
        let Some(value) = value.filter(|v| v.get() != "null") else {
            return Ok(false);
        };
        if key.len() > BusinessActionContext::MAX_CONTEXT_BYTES {
            return Err(TccError::ContextBudget);
        }
        let values = BTreeMap::from([(key.into(), value.to_owned())]);
        Self::add_context_map(context, transaction, Some(&values)).await
    }

    /// 参数为当前上下文、事务和可选批量映射；忽略 null，原子合并后最多自动报告一次。
    /// 预算/输入错误不留下半批更新；未变化不顺便发送此前已有的 updated。
    pub async fn add_context_map(
        context: &mut BusinessActionContext,
        transaction: &mut dyn TccTransaction,
        values: Option<&BTreeMap<String, Box<RawValue>>>,
    ) -> Result<bool, TccError> {
        let Some(values) = values.filter(|v| !v.is_empty()) else {
            return Ok(false);
        };
        // 先在候选值上完成所有转换与总预算校验，再发布业务上下文和网络调用。
        let mut candidate = context.clone();
        let mut changed = false;
        for (key, value) in values {
            if value.get() == "null" {
                continue;
            }
            let value = ActionContextUtil::handle_action_context(value)?;
            changed |= candidate.add_action_context(key, Some(&value))?;
        }
        *context = candidate;
        if !changed {
            return Ok(false);
        }
        context.set_updated(Some(true));
        if false {
            return Ok(false);
        }
        Self::report_context(context, transaction).await
    }

    /// 参数为当前上下文与事务；仅 updated=true 才请求一次报告，成功后清除该标志。
    /// 失败/取消不宣称成功，事务负责保留原类型化原因和封闭能力；不改变 delayReport。
    pub async fn report_context(
        context: &mut BusinessActionContext,
        transaction: &mut dyn TccTransaction,
    ) -> Result<bool, TccError> {
        if context.get_updated() != Some(true) {
            return Ok(false);
        }
        let reported = transaction.report_context(context).await?;
        if reported {
            context.set_updated(None);
        }
        Ok(reported)
    }
}
