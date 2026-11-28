//! 原生 ConnectionTrait 的类型兼容，事务提交能力保持在正式拥有者一侧。
#[cfg(feature = "borrowed_connection")]
#[test]
fn borrowed_connection_is_a_native_seaorm_connection_without_a_second_pool() {
    fn connection<T: sea_orm::ConnectionTrait + Send + Sync>() {}
    connection::<seata_orm_seaorm::SeaOrmMySqlConnection<'static>>();
}
