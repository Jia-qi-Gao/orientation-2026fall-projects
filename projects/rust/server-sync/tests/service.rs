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