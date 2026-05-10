use leptos::prelude::*;

#[component]
pub fn Tile(
    title: &'static str,
    #[prop(optional, into)] subtitle: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="tile">
            <div class="tile-title">{title}</div>
            <div class="tile-value">{children()}</div>
            {subtitle.map(|s| view! { <div class="mt-1 text-xs text-slate-500">{s}</div> })}
        </div>
    }
}
