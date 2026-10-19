use serde::Serialize;

pub mod v1;

#[derive(sqlx::Type, Serialize, Debug, Clone)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PermissionRole {
    Owner,
    Editor,
    Viewer,
}
