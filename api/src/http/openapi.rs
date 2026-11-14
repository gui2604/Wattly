use actix_web::{get, HttpResponse};

const SPEC: &str = include_str!("../../openapi.yaml");

const DOCS: &str = r##"<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="utf-8">
  <title>Wattly API</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({ url: "/openapi.yaml", dom_id: "#swagger" });
  </script>
</body>
</html>
"##;

#[get("/openapi.yaml")]
pub async fn specification() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/yaml; charset=utf-8")
        .body(SPEC)
}

#[get("/docs")]
pub async fn docs() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(DOCS)
}
