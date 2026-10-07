use rm_server_sync::Service;
use serde_json::{Value, json};

#[test]
fn input_validation_and_baseline() {
    let service = Service::default();
    assert_eq!(
        service.handle("GET", "/ping", &Value::Null, ""),
        (200, json!({"data":"pong"}))
    );
    for body in [
        Value::Null,
        json!([]),
        json!({"username":true,"password":"password1"}),
        json!({"username":"a/b","password":"password1"}),
    ] {
        assert_eq!(service.handle("POST", "/users", &body, "").0, 400);
    }
    assert_eq!(service.handle("GET", "/texts", &Value::Null, "").0, 401);
    assert_eq!(service.handle("GET", "/missing", &Value::Null, "").0, 404);
}

#[test]
fn concurrent_registration_has_one_winner() {
    let service = std::sync::Arc::new(Service::default());
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let service = service.clone();
            std::thread::spawn(move || {
                service
                    .handle(
                        "POST",
                        "/users",
                        &json!({"username":"alice","password":"password1"}),
                        "",
                    )
                    .0
            })
        })
        .collect();
    let statuses: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(statuses.iter().filter(|&&s| s == 201).count(), 1);
    assert_eq!(statuses.iter().filter(|&&s| s == 409).count(), 3);
}
#[test]
fn echo_returns_same_text() {
    let service = Service::default();
    for text in ["hello", "", "你好 Rust", "hello\nworld"] {
        let body = json!({"text": text});
        assert_eq!(
            service.handle("POST", "/echo", &body, ""),
            (200, json!({"data": text}))
        );
    }
}

#[test]
fn put_text_saves_text_for_user() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    assert_eq!(
        service.handle("POST", "/users", &account, "").0,
        201
    );
    let login = service.handle("POST", "/sessions", &account, "").1;
    let token = login["data"]["token"].as_str().unwrap();
    let authorization = format!("Bearer {token}");
    let body = json!({"text": "hello rust"});
    assert_eq!(
        service.handle(
            "PUT",
            "/texts/note",
            &body,
            &authorization
        ),
        (200, json!({"data": null}))
    );
}

#[test]
fn get_text_returns_saved_text() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    service.handle(
        "PUT",
        "/texts/note",
        &json!({"text": "hello Rust"}),
        &token,
    );
    assert_eq!(
        service.handle("GET", "/texts/note", &Value::Null, &token),
        (200, json!({"data": "hello Rust"}))
    );
}

#[test]
fn delete_text_removes_text() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 先保存文本
    service.handle(
        "PUT",
        "/texts/note",
        &json!({"text": "hello Rust"}),
        &token,
    );
    // 删除文本
    assert_eq!(
        service.handle("DELETE", "/texts/note", &Value::Null, &token),
        (200, json!({"data": null}))
    );
    // 再读取应该不存在
    assert_eq!(
        service.handle("GET", "/texts/note", &Value::Null, &token),
        (404, json!({"message": "Not found"}))
    );
}

#[test]
fn users_texts_are_isolated() {
    let service = Service::default();
    let alice = json!({
        "username": "alice",
        "password": "password1"
    });
    let bob = json!({
        "username": "bob",
        "password": "password1"
    });
    service.handle("POST", "/users", &alice, "");
    service.handle("POST", "/users", &bob, "");
    let alice_login = service.handle("POST", "/sessions", &alice, "");
    let alice_token = format!(
        "Bearer {}",
        alice_login.1["data"]["token"].as_str().unwrap()
    );
    let bob_login = service.handle("POST", "/sessions", &bob, "");
    let bob_token = format!(
        "Bearer {}",
        bob_login.1["data"]["token"].as_str().unwrap()
    );
    // Alice 保存 note
    assert_eq!(
        service.handle(
            "PUT",
            "/texts/note",
            &json!({"text": "Alice's text"}),
            &alice_token,
        )
        .0,
        200
    );
    // Bob 不应该能读到 Alice 的 note
    assert_eq!(
        service.handle("GET", "/texts/note", &Value::Null, &bob_token),
        (404, json!({"message": "Not found"}))
    );
    // Bob 保存自己的 note
    assert_eq!(
        service.handle(
            "PUT",
            "/texts/note",
            &json!({"text": "Bob's text"}),
            &bob_token,
        )
        .0,
        200
    );
    // Alice 仍然只能读到自己的内容
    assert_eq!(
        service.handle("GET", "/texts/note", &Value::Null, &alice_token),
        (200, json!({"data": "Alice's text"}))
    );
    // Bob 只能读到自己的内容
    assert_eq!(
        service.handle("GET", "/texts/note", &Value::Null, &bob_token),
        (200, json!({"data": "Bob's text"}))
    );
}


#[test]
fn text_list_updates_after_delete() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 保存三个文本
    service.handle(
        "PUT",
        "/texts/zebra",
        &json!({"text": "z"}),
        &token,
    );
    service.handle(
        "PUT",
        "/texts/apple",
        &json!({"text": "a"}),
        &token,
    );
    service.handle(
        "PUT",
        "/texts/mango",
        &json!({"text": "m"}),
        &token,
    );
    // 列表应该按名称升序
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &token),
        (
            200,
            json!({
                "data": ["apple", "mango", "zebra"]
            })
        )
    );
    // 删除 mango
    assert_eq!(
        service.handle("DELETE", "/texts/mango", &Value::Null, &token),
        (200, json!({"data": null}))
    );
    // 删除后列表应该更新
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &token),
        (
            200,
            json!({
                "data": ["apple", "zebra"]
            })
        )
    );
}

#[test]
fn delete_user_clears_account_and_texts() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    // 注册
    assert_eq!(
        service.handle("POST", "/users", &account, "").0,
        201
    );
    // 登录
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 保存文本
    assert_eq!(
        service.handle(
            "PUT",
            "/texts/note",
            &json!({"text": "hello Rust"}),
            &token,
        )
        .0,
        200
    );
    // 注销
    assert_eq!(
        service.handle("DELETE", "/users/me", &Value::Null, &token),
        (200, json!({"data": null}))
    );
    // 旧 token 应该失效
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &token).0,
        401
    );
    // 同名重新注册应该成功
    assert_eq!(
        service.handle("POST", "/users", &account, "").0,
        201
    );
    // 重新登录
    let login = service.handle("POST", "/sessions", &account, "");
    let new_token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 新账号不应该有旧文本
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &new_token),
        (200, json!({"data": []}))
    );
}

#[test]
fn expired_token_is_rejected() {
    let service = Service::new(1);
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 刚登录时应该可以访问
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &token).0,
        200
    );
    // 等待 Token 过期
    std::thread::sleep(std::time::Duration::from_secs(2));
    // 过期后应该返回 401
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &token).0,
        401
    );
}

#[test]
fn expired_token_can_be_replaced_by_login() {
    let service = Service::new(1);
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    // 第一次登录
    let login = service.handle("POST", "/sessions", &account, "");
    let old_token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 等待旧 Token 过期
    std::thread::sleep(std::time::Duration::from_secs(2));
    // 重新登录
    let login = service.handle("POST", "/sessions", &account, "");
    assert_eq!(login.0, 200);
    let new_token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // 新 Token 应该可以使用
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &new_token).0,
        200
    );
    // 旧 Token 不能使用
    assert_eq!(
        service.handle("GET", "/texts", &Value::Null, &old_token).0,
        401
    );
    // expires_in 应该存在
    assert_eq!(
        login.1["data"]["expires_in"],
        json!(1)
    );
}

#[test]
fn concurrent_text_updates_are_consistent() {
    use std::sync::Arc;
    use std::thread;
    let service = Arc::new(Service::new(0));
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    let mut workers = Vec::new();
    for i in 0..4 {
        let service = Arc::clone(&service);
        let token = token.clone();

        workers.push(thread::spawn(move || {
            service.handle(
                "PUT",
                "/texts/note",
                &json!({
                    "text": format!("text-{i}")
                }),
                &token,
            )
            .0
        }));
    }
    for worker in workers {
        assert_eq!(worker.join().unwrap(), 200);
    }
    let result = service.handle(
        "GET",
        "/texts/note",
        &Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    let text = result.1["data"].as_str().unwrap();
    assert!(
        ["text-0", "text-1", "text-2", "text-3"].contains(&text)
    );
}

#[test]
fn expired_token_cannot_delete_user() {
    let service = Service::new(1);
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    assert_eq!(login.0, 200);
    let token = login.1["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    std::thread::sleep(std::time::Duration::from_secs(2));
    let result = service.handle(
        "DELETE",
        "/users/me",
        &Value::Null,
        &format!("Bearer {}", token),
    );
    assert_eq!(result.0, 401);
    // 用户仍然存在，可以重新登录
    let login_again = service.handle("POST", "/sessions", &account, "");
    assert_eq!(login_again.0, 200);
}

#[test]
fn text_name_boundaries() {
    let service = Service::default();
    let account = json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = login.1["data"]["token"].as_str().unwrap().to_string();
    let auth = format!("Bearer {}", token);
    // 64 个字符：应该允许
    let valid_name = "a".repeat(64);
    let result = service.handle(
        "PUT",
        &format!("/texts/{}", valid_name),
        &json!({"text": "hello"}),
        &auth,
    );
    assert_eq!(result.0, 200);
    // 65 个字符：应该拒绝
    let too_long = "a".repeat(65);
    let result = service.handle(
        "PUT",
        &format!("/texts/{}", too_long),
        &json!({"text": "hello"}),
        &auth,
    );
    assert_eq!(result.0, 400);
    // 包含非法字符：应该拒绝
    let result = service.handle("PUT", "/texts/bad/name", &json!({"text": "hello"}), &auth);
    assert_eq!(result.0, 400);
    // 空名称：应该拒绝
    let result = service.handle("PUT", "/texts/", &json!({"text": "hello"}), &auth);
    assert_eq!(result.0, 400);
}