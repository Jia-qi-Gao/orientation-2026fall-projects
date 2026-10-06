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