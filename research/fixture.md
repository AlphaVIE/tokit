# Seed fixture: filtered user endpoint

This fixture is an abstract equivalence contract for syntax exploration. It is not yet runnable because the HTTP, JSON, and task APIs have no implementation.

1. Import modules `http`, `json`, and `task`.
2. Define `User` with `id: i32` and `name: str`.
3. Define generic `identity<T>(x: T) -> T` returning `x`.
4. Define `users_response(users: [User]) -> Result<Response, Error>`. Create an empty `[User]`, iterate over `users`, and append `identity(user)` when `user.id > 0`. Encode the selected array as JSON; propagate encoding errors. Return an HTTP 200 response with that body.
5. Define `main() -> Result<Unit, Error>`. Create a server, register `users_response` for `GET /users`, spawn `server.run` as a task, join it, and propagate task or server errors.

Assumed abstract APIs: `json.encode([User]) -> Result<str, Error>`, `http.response(i32,str) -> Response`, `http.server() -> Server`, `http.get(Server,str,str,handler) -> Unit`, `http.run(Server) -> Result<Unit,Error>`, `task.spawn(callable) -> Task<Result<Unit,Error>>`, and `task.join(Task<Result<Unit,Error>>) -> Result<Unit,Error>`. The implementation must define serialization of `User`, ownership/borrowing of the server, and handler data access before this becomes an executable conformance test.

The three files intentionally use unapproved spellings. Each is compared only after a reviewer confirms that the same abstract operations appear. Later fixtures must use operational tests rather than this prose contract alone.
