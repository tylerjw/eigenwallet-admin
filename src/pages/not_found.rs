use leptos::prelude::*;

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div class="text-center py-20">
            <div class="text-4xl font-semibold">"404"</div>
            <div class="mt-2 text-slate-400">"That route doesn't exist."</div>
            <a href="/" class="mt-4 inline-block text-indigo-400 hover:text-indigo-300">
                "Back to Overview"
            </a>
        </div>
    }
}
