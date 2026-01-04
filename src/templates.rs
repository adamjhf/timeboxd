use hypertext::{Raw, maud, prelude::*};

use crate::{
    countries::{COUNTRIES, get_country_name},
    models::{FilmWithReleases, ReleaseDate, ReleaseType},
};

const TAILWIND_CDN: &str = "https://cdn.tailwindcss.com";
const DATASTAR_CDN: &str =
    "https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.9/dist/datastar.js";

pub fn index_page() -> String {
    page(
        "Letterboxd Release Tracker",
        maud! {
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
                                label class="block text-sm font-medium text-gray-700" for="country-search" { "Country" }
                                div class="relative mt-2" {
                                    input
                                        type="text"
                                        id="country-search"
                                        autocomplete="off"
                                        placeholder="Search countries..."
                                        class="w-full rounded-md border border-gray-300 px-3 py-2 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                                        onkeyup="filterCountries()"
                                        onfocus="document.getElementById('country-dropdown').classList.remove('hidden')"
                                        ;
                                    input type="hidden" name="country" id="country" required;
                                    div id="country-dropdown" class="hidden absolute z-10 mt-1 w-full bg-white border border-gray-300 rounded-md shadow-lg max-h-60 overflow-y-auto" {
                                        @for country in COUNTRIES {
                                            div
                                                class="country-option px-3 py-2 hover:bg-blue-50 cursor-pointer"
                                                data-code=(country.code)
                                                data-name=(country.name)
                                                onclick=(format!("selectCountry('{}', '{}')", country.code, country.name))
                                            {
                                                (country.name) " (" (country.code) ")"
                                            }
                                        }
                                    }
                                }
                                p class="mt-2 text-xs text-gray-500" { "Select a country to see release dates for that region." }
                            }

                            button class="w-full rounded-md bg-blue-600 px-4 py-2 font-semibold text-white hover:bg-blue-700" type="submit" { "Track" }
                        }
                        (country_selector_script())
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
        maud! {
            div class="min-h-screen bg-gray-50 flex items-center justify-center" {
                div id="content" class="max-w-xl w-full px-6" {
                    div class="bg-white shadow rounded-lg p-8 text-center" {
                        div class="mx-auto h-12 w-12 rounded-full border-4 border-blue-200 border-t-blue-600 animate-spin" {}
                        h1 class="mt-6 text-xl font-semibold text-gray-900" { "Processing" }
                        p class="mt-2 text-gray-600" { "Fetching watchlist and checking release dates." }
                        p class="mt-2 text-sm text-gray-500" { "This may take a minute for large watchlists." }
                    }
                }
            }
            script { (Raw::dangerously_create(format!("
                fetch('{}')
                    .then(response => response.text())
                    .then(html => {{
                        document.getElementById('content').innerHTML = html;
                    }})
                    .catch(error => {{
                        document.getElementById('content').innerHTML = '<div class=\"bg-white shadow rounded-lg p-8\"><h1 class=\"text-2xl font-bold text-gray-900\">Error</h1><p class=\"mt-4 text-gray-700\">' + error.message + '</p></div>';
                    }});
            ", url))) }
        },
    )
}

pub fn results_fragment(username: &str, country: &str, films: &[FilmWithReleases]) -> String {
    let country_name = get_country_name(country);
    content_div(maud! {
        div class="max-w-4xl mx-auto px-4 py-4" {
            div class="flex items-start justify-between gap-4" {
                div {
                    h1 class="text-2xl font-bold text-gray-900" { "Upcoming releases" }
                    p class="mt-1 text-sm text-gray-600" { "@" (username) " · " (country_name) }
                }
                a class="text-sm text-blue-600 hover:text-blue-800" href="/" { "New search" }
            }

            @if films.is_empty() {
                div class="mt-4 bg-white shadow rounded-lg p-4" {
                    p class="text-gray-600" { "No upcoming theatrical or streaming releases found." }
                }
            } @else {
                div class="mt-4 space-y-2" {
                    @for film in films {
                        (film_card(film))
                    }
                }
            }
        }
    })
}

pub fn error_fragment(message: String) -> String {
    content_div(maud! {
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
        maud! {
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

fn page(title: &str, body: impl Renderable) -> String {
    maud! {
        !DOCTYPE
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
    .render()
    .into_inner()
}

fn content_div(inner: impl Renderable) -> String {
    maud! { div id="content" { (inner) } }.render().into_inner()
}

fn film_card(film: &FilmWithReleases) -> impl Renderable + '_ {
    maud! {
        div class="bg-white shadow rounded p-3" {
            div class="flex items-start justify-between gap-3" {
                div {
                    h2 class="text-lg font-semibold text-gray-900" {
                        (film.title)
                        @if let Some(year) = film.year {
                            span class="ml-1.5 font-normal text-gray-500" { "(" (year) ")" }
                        }
                    }
                    a class="mt-0.5 block text-xs text-gray-500 hover:text-gray-700" href=(format!("https://www.themoviedb.org/movie/{}", film.tmdb_id)) target="_blank" rel="noopener noreferrer" {
                        "TMDB"
                    }
                }
            }

            div class="mt-2 grid gap-3 md:grid-cols-2" {
                (release_list("Theatrical", &film.theatrical, ReleaseType::Theatrical))
                (release_list("Streaming", &film.streaming, ReleaseType::Digital))
            }
        }
    }
}

fn release_list<'a>(
    label: &'a str,
    releases: &'a [ReleaseDate],
    kind: ReleaseType,
) -> impl Renderable + 'a {
    let border = match kind {
        ReleaseType::Theatrical => "border-purple-500",
        ReleaseType::Digital => "border-blue-500",
    };

    maud! {
        div class=(format!("border-l-3 {} pl-2.5", border)) {
            h3 class="text-xs font-semibold text-gray-700 uppercase tracking-wide" { (label) }
            @if releases.is_empty() {
                p class="mt-1 text-sm text-gray-500" { "—" }
            } @else {
                ul class="mt-1 space-y-0.5" {
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

fn country_selector_script() -> impl Renderable {
    maud! {
        script {
            (Raw::dangerously_create(r#"
                function selectCountry(code, name) {
                    document.getElementById('country').value = code;
                    document.getElementById('country-search').value = name;
                    document.getElementById('country-dropdown').classList.add('hidden');
                }

                function filterCountries() {
                    const input = document.getElementById('country-search');
                    const filter = input.value.toLowerCase();
                    const dropdown = document.getElementById('country-dropdown');
                    const options = dropdown.getElementsByClassName('country-option');

                    let hasVisible = false;
                    for (let i = 0; i < options.length; i++) {
                        const name = options[i].getAttribute('data-name').toLowerCase();
                        const code = options[i].getAttribute('data-code').toLowerCase();
                        if (name.includes(filter) || code.includes(filter)) {
                            options[i].style.display = '';
                            hasVisible = true;
                        } else {
                            options[i].style.display = 'none';
                        }
                    }

                    if (hasVisible) {
                        dropdown.classList.remove('hidden');
                    }
                }

                document.addEventListener('click', function(event) {
                    const dropdown = document.getElementById('country-dropdown');
                    const searchInput = document.getElementById('country-search');
                    if (dropdown && searchInput && !dropdown.contains(event.target) && event.target !== searchInput) {
                        dropdown.classList.add('hidden');
                    }
                });
            "#))
        }
    }
}
