#[derive(Clone, Debug)]
pub enum SLSKEvents {
    TryLogin { username: String, password: String },
    LoginResult(bool, Option<String>),
    Quit,
}
