use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::models::{FilmWithReleases, ReleaseDate, ReleaseType};

const TAILWIND_CDN: &str = "https://cdn.tailwindcss.com";
const DATASTAR_CDN: &str =
    "https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.9/dist/datastar.js";

pub fn index_page() -> String {
    page(
        "Letterboxd Release Tracker",
        html! {
            div class="min-h-screen bg-gray-50" {
                div class="max-w-2xl mx-auto px-6 py-12" {
                    div class="bg-white shadow rounded-lg p-8" {
                        h1 class="text-3xl font-bold text-gray-900" { "Letterboxd Release Tracker" }
                        p class="mt-2 text-gray-600" { "Upcoming theatrical and streaming releases from your watchlist." }

                        form class="mt-8 space-y-6" method="post" action="/track" {
                            div {
                                label class="block text-sm font-medium text-gray-700" for="username" { "Letterboxd username" }
                                input class="mt-2 w-full rounded-md border border-gray-300 px-3 py-2 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500" name="username" id="username" required;
                            }

                            div {
                                label class="block text-sm font-medium text-gray-700" for="country" { "Country code" }
                                input class="mt-2 w-full rounded-md border border-gray-300 px-3 py-2 uppercase focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500" name="country" id="country" minlength="2" maxlength="2" pattern="[A-Za-z]{2}" required;
                                p class="mt-2 text-xs text-gray-500" { "Use ISO 3166-1 alpha-2 (e.g. US, GB, AU)." }
                            }

                            button class="w-full rounded-md bg-blue-600 px-4 py-2 font-semibold text-white hover:bg-blue-700" type="submit" { "Track" }
                        }
                    }
                }
            }
        },
    )
}

pub fn processing_page(username: &str, country: &str) -> String {
    let url = format!(
        "/process?username={}&country={}",
        urlencoding::encode(username),
        urlencoding::encode(country)
    );

    page(
        "Processing",
        html! {
            div class="min-h-screen bg-gray-50 flex items-center justify-center" {
                div id="content" class="max-w-xl w-full px-6" data-indicator:fetching data-init=(PreEscaped(format!("@get('{}')", url))) {
                    div class="bg-white shadow rounded-lg p-8 text-center" {
                        div class="mx-auto h-12 w-12 rounded-full border-4 border-blue-200 border-t-blue-600 animate-spin" {};
                        h1 class="mt-6 text-xl font-semibold text-gray-900" { "Processing" }
                        p class="mt-2 text-gray-600" { "Fetching watchlist and checking release dates." }
                        p class="mt-2 text-sm text-gray-500" { "This may take a minute for large watchlists." }
                    }
                }
            }
        },
    )
}

pub fn results_fragment(username: &str, country: &str, films: &[FilmWithReleases]) -> String {
    content_div(html! {
        div class="max-w-4xl mx-auto px-6 py-10" {
            div class="flex items-start justify-between gap-6" {
                div {
                    h1 class="text-3xl font-bold text-gray-900" { "Upcoming releases" }
                    p class="mt-2 text-gray-600" { "@" (username) " · " (country) }
                }
                a class="text-sm text-blue-600 hover:text-blue-800" href="/" { "New search" }
            }

            @if films.is_empty() {
                div class="mt-10 bg-white shadow rounded-lg p-8" {
                    p class="text-gray-600" { "No upcoming theatrical or streaming releases found." }
                }
            } @else {
                div class="mt-10 space-y-4" {
                    @for film in films {
                        (film_card(film))
                    }
                }
            }
        }
    })
}

pub fn error_fragment(message: String) -> String {
    content_div(html! {
        div class="max-w-2xl mx-auto px-6 py-12" {
            div class="bg-white shadow rounded-lg p-8" {
                h1 class="text-2xl font-bold text-gray-900" { "Error" }
                p class="mt-4 text-gray-700" { (message) }
                a class="mt-6 inline-block text-blue-600 hover:text-blue-800" href="/" { "Back" }
            }
        }
    })
}

pub fn error_page(message: String) -> String {
    page(
        "Error",
        html! {
            div class="min-h-screen bg-gray-50 flex items-center justify-center" {
                div class="max-w-xl w-full px-6" {
                    div class="bg-white shadow rounded-lg p-8" {
                        h1 class="text-2xl font-bold text-gray-900" { "Error" }
                        p class="mt-4 text-gray-700" { (message) }
                        a class="mt-6 inline-block text-blue-600 hover:text-blue-800" href="/" { "Back" }
                    }
                }
            }
        },
    )
}

fn page(title: &str, body: Markup) -> String {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                script src=(TAILWIND_CDN) {}
                script type="module" src=(DATASTAR_CDN) {}
            }
            body { (body) }
        }
    }
    .into_string()
}

fn content_div(inner: Markup) -> String {
    html! { div id="content" { (inner) } }.into_string()
}

fn film_card(film: &FilmWithReleases) -> Markup {
    html! {
        div class="bg-white shadow rounded-lg p-6" {
            div class="flex items-start justify-between gap-4" {
                div {
                    h2 class="text-xl font-semibold text-gray-900" {
                        (film.title)
                        @if let Some(year) = film.year {
                            span class="ml-2 font-normal text-gray-500" { "(" (year) ")" }
                        }
                    }
                    a class="mt-1 block text-sm text-gray-500 hover:text-gray-700" href=(format!("https://www.themoviedb.org/movie/{}", film.tmdb_id)) target="_blank" rel="noopener noreferrer" {
                        "TMDB"
                    }
                }
            }

            div class="mt-4 grid gap-4 md:grid-cols-2" {
                (release_list("Theatrical", &film.theatrical, ReleaseType::Theatrical))
                (release_list("Streaming", &film.streaming, ReleaseType::Digital))
            }
        }
    }
}

fn release_list(label: &str, releases: &[ReleaseDate], kind: ReleaseType) -> Markup {
    let border = match kind {
        ReleaseType::Theatrical => "border-purple-500",
        ReleaseType::Digital => "border-blue-500",
    };

    html! {
        div class=(format!("border-l-4 {} pl-4", border)) {
            h3 class="text-sm font-semibold text-gray-700" { (label) }
            @if releases.is_empty() {
                p class="mt-2 text-sm text-gray-500" { "—" }
            } @else {
                ul class="mt-2 space-y-1" {
                    @for rel in releases {
                        li class="text-sm text-gray-700" {
                            span class="font-medium" { (format_date(rel)) }
                            @if let Some(note) = &rel.note {
                                span class="text-gray-500" { " · " (note) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_date(rel: &ReleaseDate) -> String {
    rel.date.strftime("%Y-%m-%d").to_string()
}
