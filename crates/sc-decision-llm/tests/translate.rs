fn answer_text() -> String {
    json!({
        "answers": {
            "q": {
                "choice": "no",
                "confidence": 0.7,
                "probabilities": { "yes": 0.3, "no": 0.7 }
            }
        }
    })
    .to_string()
}
