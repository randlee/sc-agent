    fn http_request(&self, endpoint: &Endpoint, body: &Arc<str>) -> HttpRequest {
        wire::unsigned_headers(self.provider.kind)
            .into_iter()
            .fold(
                HttpRequest::new(Method::Post, endpoint.clone(), Arc::clone(body)),
                |request, (name, value)| request.with_header(name, value),
            )
            .with_secret(wire::secret_header(
                self.provider.kind,
                self.api_key.clone(),
            ))
    }
