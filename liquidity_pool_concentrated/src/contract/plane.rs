use super::*;

#[contractimpl]
impl Plane for ConcentratedLiquidityPool {
    fn init_pools_plane(e: Env, plane: Address) {
        set_plane(&e, &plane);
    }

    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        set_plane(&e, &plane);
    }

    fn get_pools_plane(e: Env) -> Address {
        get_plane(&e)
    }

    fn backfill_plane_data(e: Env) {
        update_plane(&e);
    }
}
