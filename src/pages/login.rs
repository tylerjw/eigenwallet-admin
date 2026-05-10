use leptos::prelude::*;

#[server(name = Login, prefix = "/api/auth", endpoint = "login")]
pub async fn login(password: String) -> Result<bool, ServerFnError> {
    use crate::server::auth;
    let state = crate::server::ssr_state()?;
    let session: tower_sessions::Session = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(format!("session extract: {e}")))?;
    let ok = auth::verify_password(&state, &password)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if ok {
        auth::mark_authed(&session)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        leptos_axum::redirect("/");
    }
    Ok(ok)
}

#[server(name = Logout, prefix = "/api/auth", endpoint = "logout")]
pub async fn logout() -> Result<(), ServerFnError> {
    let session: tower_sessions::Session = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(format!("session extract: {e}")))?;
    crate::server::auth::clear(&session).await;
    leptos_axum::redirect("/login");
    Ok(())
}

#[component]
pub fn LoginPage() -> impl IntoView {
    let action = ServerAction::<Login>::new();
    let pending = action.pending();
    let value = action.value();

    view! {
        <div class="max-w-sm mx-auto mt-16 tile">
            <h1 class="text-xl font-semibold mb-4">"Sign in"</h1>
            <ActionForm action=action attr:class="flex flex-col gap-3">
                <label class="text-xs uppercase tracking-wide text-slate-400">
                    "Password"
                    <input type="password" name="password" class="input mt-1" autofocus required/>
                </label>
                <button type="submit" class="btn" disabled=move || pending.get()>
                    {move || if pending.get() { "Signing in…" } else { "Sign in" }}
                </button>
                {move || match value.get() {
                    Some(Ok(false)) => view! {
                        <div class="text-rose-300 text-sm">"Incorrect password"</div>
                    }.into_any(),
                    Some(Err(e)) => view! {
                        <div class="text-rose-300 text-sm">{e.to_string()}</div>
                    }.into_any(),
                    _ => ().into_any(),
                }}
            </ActionForm>
        </div>
    }
}
