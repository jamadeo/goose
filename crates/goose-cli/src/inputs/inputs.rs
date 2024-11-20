use cliclack::{input, password};

pub fn get_env_value_or_input(env_name: &str, input_prompt: &str, mask: bool) -> String {
    if let Ok(value) = std::env::var(env_name) {
        return value;
    }

    let input_value = if mask {
        password(input_prompt).mask('▪').interact().unwrap()
    } else {
        input(input_prompt).interact().unwrap()
    };

    std::env::set_var(env_name, &input_value);
    input_value
}

pub fn get_user_input(message: &str, default_value: &str) -> std::io::Result<String> {
    input(message).default_input(default_value).interact()
}
