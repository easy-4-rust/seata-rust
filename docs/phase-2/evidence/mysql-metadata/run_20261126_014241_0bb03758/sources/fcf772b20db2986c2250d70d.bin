/// 控制事务内部的未发布判定；InsertedGuard尚未提交，禁止作为持久围栏事实暴露。
/// 对应 Java: AbstractUndoLogManager#undo的Normal/GlobalFinished/空行分支。
pub(crate) enum UndoFenceDecision {
    InsertedGuard,
    ExistingGuard,
    NormalRow,
}
