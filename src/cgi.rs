use std::collections::HashMap;

pub struct CgiHandler {
    pub script_path: String,
    pub env_vars: HashMap<String, String>,
}

impl CgiHandler {
    pub fn new(script_path: String) -> Self {
        CgiHandler {
            script_path,
            env_vars: HashMap::new(),
        }
    }
}