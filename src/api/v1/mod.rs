mod admin;
mod group;
mod task;
mod user;

use rkt::Route;

pub fn routes() -> Vec<Route> {
    rkt::routes![
        user::register,
        user::login,
        user::get_self,
        user::logout,
        user::get_groups,
        group::create_group,
        group::get_groups,
        group::delete_group,
        group::join_group,
        group::add_to_group,
        task::create_task,
        task::delete_task,
        task::get_own_tasks,
        task::get_my_tasks,
        task::get_group_tasks,
        admin::get_users,
        admin::delete_user,
        admin::create_invite,
        admin::get_groups,
    ]
}
