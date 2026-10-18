pub mod v1;

#[derive(sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}
