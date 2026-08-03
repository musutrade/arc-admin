//! 生成 argon2 密码哈希（PHC 字符串），用于迁移中的种子账号
//! 用法：cargo run --bin gen_password_hash -- <password>

use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};

fn main() {
    let password = std::env::args()
        .nth(1)
        .expect("用法: gen_password_hash <password>");
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("密码哈希生成失败")
        .to_string();
    println!("{hash}");
}
