use super::fixture;
use seata_tcc::{
    BusinessActionContext, BusinessActionContextUtil, TccBusinessAction, TccError, TccTransaction,
};
use std::{collections::BTreeMap, future::Future, pin::Pin};
/// 真实 Try DML 后共享新参数；立即模式先报告一次，再由最终延迟路径报告后续变更。
/// 对应 Java: BusinessActionContextUtil#addContext 的测试业务扩展。
#[derive(Debug)]
pub struct TccDynamicAction {
    /// 是否在 Try 尚未返回时显式等待首次报告。
    pub immediate: bool,
    /// 是否只变更 Double 比较会合并、但原字节不同的小数。
    pub precision_only: bool,
    /// 是否故意捕获报告错误后仍返回业务成功，验证事务仍被封闭。
    pub catch_report_error: bool,
    /// 业务更新参数后是否返回 false 或抛错，验证 finally 报告与业务回滚边界。
    pub failure_mode: Option<&'static str>,
}
impl TccBusinessAction for TccDynamicAction {
    fn try_action<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .try_action(context, transaction)
                .await
        })
    }
    fn try_action_with_context<'a>(
        &'a self,
        context: &'a mut BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .try_action(context, transaction)
                .await?;
            if self.precision_only {
                let precise = serde_json::value::RawValue::from_string(
                    "1.23000000000000000000000000002".into(),
                )
                .map_err(|_| TccError::Business)?;
                context.set_updated(None);
                if context.add_action_context("precise", Some(&precise))?
                    || context.get_updated().is_some()
                {
                    return Err(TccError::Business);
                }
                return Ok(true);
            }
            let amount = serde_json::value::RawValue::from_string("9".into())
                .map_err(|_| TccError::Business)?;
            let precise =
                serde_json::value::RawValue::from_string("1.23000000000000000000000000001".into())
                    .map_err(|_| TccError::Business)?;
            context.set_delay_report(Some(!self.immediate));
            let values = BTreeMap::from([
                ("amount".into(), amount.clone()),
                ("precise".into(), precise),
                (
                    "object".into(),
                    serde_json::value::RawValue::from_string(
                        r#"{"a":1,"precise":1.23000000000000000000000000001}"#.into(),
                    )
                    .map_err(|_| TccError::Business)?,
                ),
                (
                    "array".into(),
                    serde_json::value::RawValue::from_string("[1,2]".into())
                        .map_err(|_| TccError::Business)?,
                ),
            ]);
            let reported =
                BusinessActionContextUtil::add_context_map(context, transaction, Some(&values))
                    .await;
            if self.immediate && self.catch_report_error {
                let _ = reported;
                return Ok(true);
            }
            let reported = reported?;
            if self.immediate {
                if !reported {
                    return Err(TccError::Business);
                }
                context.set_delay_report(Some(true));
                if BusinessActionContextUtil::add_context(
                    context,
                    transaction,
                    "final",
                    Some(&amount),
                )
                .await?
                {
                    return Err(TccError::Business);
                }
            } else if reported {
                return Err(TccError::Business);
            }
            match self.failure_mode {
                Some("false") => Ok(false),
                Some("error") => Err(TccError::Business),
                _ => Ok(true),
            }
        })
    }
    fn confirm<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .confirm(context, transaction)
                .await
        })
    }
    fn cancel<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .cancel(context, transaction)
                .await
        })
    }
}
