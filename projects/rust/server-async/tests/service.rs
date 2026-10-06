use rm_server_async::Service;
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
        let result = service.handle(
            "POST",
            "/echo",
            &serde_json::json!({
                "text": text
            }),
            "",
        );
        assert_eq!(result.0, 200);
        assert_eq!(result.1["data"], text);
    }
}

#[test]
fn echo_rejects_invalid_input() {
    let service = Service::default();
    let result = service.handle(
        "POST",
        "/echo",
        &serde_json::json!({}),
        "",
    );
    assert_eq!(result.0, 400);
    let result = service.handle(
        "POST",
        "/echo",
        &serde_json::json!({
            "text": 123
        }),
        "",
    );
    assert_eq!(result.0, 400);
    let result = service.handle(
        "POST",
        "/echo",
        &serde_json::json!({
            "text": "hello",
            "extra": "value"
        }),
        "",
    );
    assert_eq!(result.0, 400);
}

#[test]
fn put_text_saves_text_for_user() {
    let service = Service::default();
    let account = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    assert_eq!(
        service.handle("POST", "/users", &account, "").0,
        201
    );
    let login = service.handle("POST", "/sessions", &account, "");
    assert_eq!(login.0, 200);
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    let result = service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "hello rust"
        }),
        &token,
    );
    assert_eq!(result.0, 200);
}

#[test]
fn get_text_returns_saved_text() {
    let service = Service::default();
    let account = serde_json::json!({
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
        &serde_json::json!({
            "text": "hello rust"
        }),
        &token,
    );
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], "hello rust");
}

#[test]
fn put_text_overwrites_existing_text() {
    let service = Service::default();
    let account = serde_json::json!({
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
        &serde_json::json!({
            "text": "old text"
        }),
        &token,
    );
    service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "new text"
        }),
        &token,
    );
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], "new text");
}

#[test]
fn text_requires_login() {
    let service = Service::default();
    let result = service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "hello"
        }),
        "",
    );
    assert_eq!(result.0, 401);
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        "",
    );
    assert_eq!(result.0, 401);
}

#[test]
fn get_missing_text_returns_404() {
    let service = Service::default();
    let account = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    let result = service.handle(
        "GET",
        "/texts/missing",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 404);
}

#[test]
fn delete_text_removes_text() {
    let service = Service::default();

    let account = serde_json::json!({
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
        &serde_json::json!({
            "text": "hello rust"
        }),
        &token,
    );
    let result = service.handle(
        "DELETE",
        "/texts/note",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 404);
}

#[test]
fn delete_missing_text_returns_404() {
    let service = Service::default();
    let account = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    let result = service.handle(
        "DELETE",
        "/texts/missing",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 404);
}

#[test]
fn text_list_updates_after_delete() {
    let service = Service::default();
    let account = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let login = service.handle("POST", "/sessions", &account, "");
    let token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    for name in ["zebra", "apple", "mango"] {
        service.handle(
            "PUT",
            &format!("/texts/{name}"),
            &serde_json::json!({
                "text": name
            }),
            &token,
        );
    }
    let result = service.handle(
        "GET",
        "/texts",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(
        result.1["data"],
        serde_json::json!(["apple", "mango", "zebra"])
    );
    service.handle(
        "DELETE",
        "/texts/mango",
        &serde_json::Value::Null,
        &token,
    );
    let result = service.handle(
        "GET",
        "/texts",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(
        result.1["data"],
        serde_json::json!(["apple", "zebra"])
    );
}

#[test]
fn users_texts_are_isolated() {
    let service = Service::default();
    let alice = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    let bob = serde_json::json!({
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
    service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "alice text"
        }),
        &alice_token,
    );
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &bob_token,
    );
    assert_eq!(result.0, 404);
    let result = service.handle(
        "GET",
        "/texts",
        &serde_json::Value::Null,
        &bob_token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], serde_json::json!([]));
    service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "bob text"
        }),
        &bob_token,
    );
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &alice_token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], "alice text");
}

#[test]
fn deleting_one_users_text_does_not_affect_another() {
    let service = Service::default();
    let alice = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    let bob = serde_json::json!({
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
    service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "alice text"
        }),
        &alice_token,
    );
    service.handle(
        "PUT",
        "/texts/note",
        &serde_json::json!({
            "text": "bob text"
        }),
        &bob_token,
    );
    let result = service.handle(
        "DELETE",
        "/texts/note",
        &serde_json::Value::Null,
        &bob_token,
    );
    assert_eq!(result.0, 200);
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &alice_token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], "alice text");
}

#[test]
fn delete_user_clears_account_and_texts() {
    let service = Service::default();
    let account = serde_json::json!({
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
        &serde_json::json!({
            "text": "alice text"
        }),
        &token,
    );
    let result = service.handle(
        "DELETE",
        "/users/me",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 200);
    // Old token should no longer work.
    let result = service.handle(
        "GET",
        "/texts/note",
        &serde_json::Value::Null,
        &token,
    );
    assert_eq!(result.0, 401);
    // Re-register the same username.
    let result = service.handle("POST", "/users", &account, "");
    assert_eq!(result.0, 201);
    let login = service.handle("POST", "/sessions", &account, "");
    assert_eq!(login.0, 200);
    let new_token = format!(
        "Bearer {}",
        login.1["data"]["token"].as_str().unwrap()
    );
    // Old text should be gone.
    let result = service.handle(
        "GET",
        "/texts",
        &serde_json::Value::Null,
        &new_token,
    );
    assert_eq!(result.0, 200);
    assert_eq!(result.1["data"], serde_json::json!([]));
}

#[test]
fn login_returns_expires_in() {
    let service = Service::default();
    let account = serde_json::json!({
        "username": "alice",
        "password": "password1"
    });
    service.handle("POST", "/users", &account, "");
    let result = service.handle(
        "POST",
        "/sessions",
        &account,
        "",
    );
    assert_eq!(result.0, 200);
    let expires_in = result.1["data"]["expires_in"]
        .as_u64()
        .unwrap();
    assert!(expires_in > 0);
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
    assert_eq!(login.0, 200);
    let token = login.1["data"]["token"].as_str().unwrap().to_string();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let result = service.handle("GET", "/texts", &json!({}), &format!("Bearer {}", token));
    assert_eq!(result.0, 401);
}