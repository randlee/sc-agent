fn secs(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

fn request() -> HttpRequest {
    let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, KEY.to_owned()).expect("key");
    HttpRequest::new(
        Method::Post,
        Endpoint::parse("https://api.example.test/v1/x").expect("https"),
        Arc::from("{\"same\":\"body\"}"),
    )
    .with_secret(SecretHeader::bearer(key))
}

fn busy(status: u16, retry_after: Option<&str>) -> HttpResponse {
    let headers = retry_after
        .map(|value| vec![("Retry-After".to_owned(), value.to_owned())])
        .unwrap_or_default();
    HttpResponse::new(status, headers, Vec::new())
}

fn ok() -> HttpResponse {
    HttpResponse::new(200, Vec::new(), b"{}".to_vec())
}
