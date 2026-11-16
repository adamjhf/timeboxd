use hypertext::{Raw, maud, prelude::*};
use jiff::civil::Date;

use crate::{
    countries::{COUNTRIES, get_country_name},
    models::{
        FilmWithReleases, ProviderType, ReleaseCategory, ReleaseDate, ReleaseType, WatchProvider,
        WishlistFilm,
    },
    watchlist::SavedWatchlist,
};

const TAILWIND_CDN: &str = "https://cdn.tailwindcss.com";
const DATASTAR_CDN: &str =
    "https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.9/dist/datastar.js";

const LETTERBOXD_EXPORT_URL: &str = "https://letterboxd.com/user/exportdata/";
const STALE_AFTER_DAYS: i64 = 14;

pub fn results_url(token: &str, country: &str) -> String {
    format!(
        "/release-dates?watchlist={}&country={}",
        urlencoding::encode(token),
        urlencoding::encode(country)
    )
}

pub fn index_page(saved: Option<&SavedWatchlist>, saved_country: Option<&str>) -> String {
    let country_name = saved_country.map(get_country_name);
    let saved_link = saved.zip(saved_country).map(|(w, c)| (w, results_url(&w.token, c)));

    page(
        "Timeboxd - upcoming film releases from your Letterboxd watchlist",
        maud! {
            div class="min-h-screen bg-slate-900" {
                div class="max-w-2xl mx-auto px-4 py-12 sm:px-6" {
                    div class="bg-slate-800 shadow-xl rounded-lg p-6 sm:p-8 border border-slate-700" {
                        h1 class="text-2xl sm:text-3xl font-bold text-slate-100" { "Timeboxd" }
                        p class="mt-2 text-slate-400" { "Upcoming film release dates for your Letterboxd watchlist." }

                        @if let Some((watchlist, link)) = &saved_link {
                            div class="mt-6 rounded-md border border-slate-600 bg-slate-700/50 p-4 flex flex-col sm:flex-row sm:items-center gap-3 justify-between" {
                                div class="min-w-0" {
                                    p class="text-sm font-medium text-slate-200" { "Your saved watchlist" }
                                    p class="mt-0.5 text-sm text-slate-400" { (watchlist_summary(watchlist)) }
                                }
                                a class="flex-shrink-0 rounded-md bg-orange-600 px-4 py-2 text-center font-semibold text-white hover:bg-orange-700" href=(link) { "Show release dates" }
                            }
                        }

                        form id="upload-form" class="mt-8 space-y-6" method="post" action="/upload" enctype="multipart/form-data" onsubmit="startUpload()" {
                            div {
                                label class="block text-sm font-medium text-slate-300" for="country-search" { "Country" }
                                div class="relative mt-2" {
                                    input
                                        type="text"
                                        id="country-search"
                                        autocomplete="off"
                                        class="w-full rounded-md border border-slate-600 bg-slate-700 text-slate-100 px-3 py-2 placeholder-slate-400 focus:border-orange-500 focus:outline-none focus:ring-1 focus:ring-orange-500"
                                        value=[country_name]
                                        onkeyup="filterCountries()"
                                        oninput="validateForm()"
                                        onkeydown="handleCountryKeydown(event)"
                                        onfocus="document.getElementById('country-dropdown').classList.remove('hidden')"
                                        ;
                                    input type="hidden" name="country" id="country" value=[saved_country];
                                    div id="country-dropdown" class="hidden absolute z-10 mt-1 w-full bg-slate-700 border border-slate-600 rounded-md shadow-lg max-h-60 overflow-y-auto" {
                                        @for country in COUNTRIES {
                                            div
                                                class="country-option px-3 py-2 text-slate-200 hover:bg-slate-600 cursor-pointer focus:bg-orange-900 focus:outline-none"
                                                data-code=(country.code)
                                                data-name=(country.name)
                                                data-timezones=(country.timezones.join(" "))
                                                tabindex="-1"
                                                onclick=(format!("selectCountry('{}', '{}')", country.code, country.name))
                                            {
                                                (country.name)
                                            }
                                        }
                                    }
                                }
                                p class="mt-2 text-xs text-slate-500" { "Select a country to see release dates for that region." }
                            }

                            div {
                                p class="block text-sm font-medium text-slate-300" {
                                    @if saved.is_some() { "Upload a newer watchlist" } @else { "Your Letterboxd watchlist" }
                                }
                                ol class="mt-3 space-y-4" {
                                    li class="flex gap-3" {
                                        span class="flex-shrink-0 h-6 w-6 rounded-full bg-slate-700 text-xs font-semibold text-slate-200 flex items-center justify-center" { "1" }
                                        div class="min-w-0" {
                                            a class="inline-block rounded-md border border-orange-600 px-3 py-1.5 text-sm font-semibold text-orange-400 hover:bg-orange-600 hover:text-white" href=(LETTERBOXD_EXPORT_URL) target="_blank" rel="noopener noreferrer" {
                                                "Open your Letterboxd export page ↗"
                                            }
                                            p class="mt-1.5 text-sm text-slate-400" { "Sign in if asked, then click " span class="text-slate-200" { "Export Data" } ". A ZIP file downloads in a few seconds." }
                                        }
                                    }
                                    li class="flex gap-3" {
                                        span class="flex-shrink-0 h-6 w-6 rounded-full bg-slate-700 text-xs font-semibold text-slate-200 flex items-center justify-center" { "2" }
                                        div class="min-w-0 flex-1" {
                                            div id="drop-zone" class="relative rounded-md border-2 border-dashed border-slate-600 bg-slate-700/40 px-4 py-6 text-center hover:border-orange-500 transition-colors" {
                                                input
                                                    type="file"
                                                    name="file"
                                                    id="file"
                                                    accept=".zip,.csv,application/zip,text/csv"
                                                    class="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                                                    aria-label="Letterboxd export ZIP or watchlist.csv"
                                                    onchange="fileChosen()"
                                                    ondragenter="setDragging(true)"
                                                    ondragleave="setDragging(false)"
                                                    ondrop="setDragging(false)";
                                                p id="file-name" class="font-medium text-slate-200" { "Drop the ZIP here, or click to choose it" }
                                                p class="mt-1 text-xs text-slate-400" { "No need to unzip it. Just watchlist.csv works too." }
                                            }
                                        }
                                    }
                                }
                            }

                            button id="submit-button" class="w-full rounded-md bg-orange-600 px-4 py-2 font-semibold text-white hover:bg-orange-700 focus:outline-none focus:ring-1 focus:ring-orange-500 disabled:opacity-50 disabled:cursor-not-allowed" type="submit" disabled { "Find release dates" }

                            p class="text-xs text-slate-500" { "Privacy: only watchlist.csv and your username are read from the ZIP. Your email, diary and reviews are ignored and never stored. To share even less, unzip the export and upload just watchlist.csv." }
                        }
                        (country_selector_script())
                    }
                }
            }
        },
    )
}

pub fn processing_page(token: &str, country: &str) -> String {
    let url = format!(
        "/process?watchlist={}&country={}",
        urlencoding::encode(token),
        urlencoding::encode(country)
    );

    page(
        "Upcoming film releases - Timeboxd",
        maud! {
            div id="content" class="min-h-screen bg-slate-900 flex items-center justify-center" {
                div class="max-w-xl w-full px-6" {
                    div class="bg-slate-800 shadow-xl rounded-lg p-8 text-center border border-slate-700" {
                        div class="mx-auto h-12 w-12 rounded-full border-4 border-slate-700 border-t-orange-600 animate-spin" {}
                        h1 class="mt-6 text-xl font-semibold text-slate-100" { "Processing" }
                        p class="mt-2 text-slate-400" { "Matching your watchlist and checking release dates." }
                        p class="mt-2 text-sm text-slate-500" { "The first run can take a minute for large watchlists." }
                    }
                }
            }
            script { (Raw::dangerously_create(format!("
                fetch('{}')
                    .then(response => response.text())
                    .then(html => {{
                        document.getElementById('content').outerHTML = html;
                    }})
                    .catch(error => {{
                        document.querySelector('#content .max-w-xl').innerHTML = '<div class=\"bg-slate-800 shadow-xl rounded-lg p-8 border border-slate-700\"><h1 class=\"text-2xl font-bold text-slate-100\">Error</h1><p class=\"mt-4 text-slate-400\">' + error.message + '</p></div>';
                    }});
            ", url))) }
        },
    )
}

fn watchlist_summary(watchlist: &SavedWatchlist) -> String {
    let who = watchlist.username.as_ref().map(|u| format!("@{u} · ")).unwrap_or_default();
    let count = watchlist.films.len();
    let plural = if count == 1 { "" } else { "s" };
    format!("{who}{count} film{plural} · uploaded {}", format_timestamp(watchlist.uploaded_at))
}

fn format_timestamp(ts: jiff::Timestamp) -> String {
    ts.to_zoned_datetime(jiff::tz::TimeZone::system()).strftime("%-d %b %Y").to_string()
}

pub fn results_fragment(
    watchlist: &SavedWatchlist,
    country: &str,
    films: &[FilmWithReleases],
    unmatched: &[WishlistFilm],
) -> String {
    let country_name = get_country_name(country);
    let age_days =
        (jiff::Timestamp::now().as_second() - watchlist.uploaded_at.as_second()) / 86_400;

    let today: Date = jiff::ZonedDateTime::now().into();
    let min_year = today.year() - 1;

    let mut unmatched: Vec<_> =
        unmatched.iter().filter(|f| f.year.is_none_or(|y| y >= min_year)).collect();
    unmatched.sort_by(|a, b| a.title.cmp(&b.title));

    let recent_films = |category| {
        films
            .iter()
            .filter(move |f| f.category == category)
            .filter(|f| f.year.is_some_and(|y| y >= min_year))
    };
    let dates = |f: &FilmWithReleases| {
        f.theatrical.iter().chain(&f.streaming).map(|r| r.date).collect::<Vec<_>>()
    };

    let mut upcoming: Vec<_> = recent_films(ReleaseCategory::LocalUpcoming)
        .filter_map(|f| {
            let dates = dates(f);
            let next = dates.iter().filter(|d| **d >= today).min().or(dates.iter().min());
            next.map(|d| (*d, f))
        })
        .collect();
    upcoming.sort_by(|(ad, a), (bd, b)| ad.cmp(bd).then(a.title.cmp(&b.title)));

    let mut released: Vec<_> = recent_films(ReleaseCategory::LocalAlreadyAvailable)
        .filter_map(|f| {
            let dates = dates(f);
            let latest = dates.iter().filter(|d| **d < today).max().or(dates.iter().max());
            latest.map(|d| (*d, f))
        })
        .collect();
    released.sort_by(|(ad, a), (bd, b)| bd.cmp(ad).then(a.title.cmp(&b.title)));

    let mut no_dates: Vec<_> = films
        .iter()
        .filter(|f| f.category == ReleaseCategory::NoReleases)
        .filter(|f| f.year.is_none_or(|y| y >= min_year))
        .collect();
    no_dates.sort_by(|a, b| a.year.cmp(&b.year).then(a.title.cmp(&b.title)));

    let tabs = [
        ("upcoming", "Upcoming", upcoming.len()),
        ("released", "Released", released.len()),
        ("no-dates", "No dates", no_dates.len() + unmatched.len()),
    ];
    let tabs: Vec<_> = tabs.into_iter().filter(|(_, _, count)| *count > 0).collect();
    let tab_css: String = tabs
        .iter()
        .map(|(id, ..)| {
            format!(
                "#tab-{id}:checked~.tab-panels>.panel-{id}{{display:block}}#tab-{id}:checked~nav \
                 label[for=tab-{id}]{{background:#334155;color:#f8fafc}}#tab-{id}:\
                 focus-visible~nav label[for=tab-{id}]{{outline:2px solid #f97316}}"
            )
        })
        .collect();

    let uses_fallback = films
        .iter()
        .flat_map(|f| f.theatrical.iter().chain(&f.streaming))
        .any(|r| r.note.as_deref().is_some_and(|n| n != country));
    let fallback = if country == "NZ" { "Australian or US" } else { "US" };

    content_div(maud! {
        div class="max-w-3xl mx-auto px-3 pt-5 pb-10 sm:px-6" {
            header class="flex items-start justify-between gap-4" {
                div class="min-w-0" {
                    a class="text-xl font-bold text-slate-100 hover:text-orange-400" href="/" { "Timeboxd" }
                    p class="mt-0.5 text-sm text-slate-400" {
                        @if let Some(username) = &watchlist.username {
                            a class="text-slate-300 hover:text-orange-400" href=(format!("https://letterboxd.com/{}/", urlencoding::encode(username))) target="_blank" rel="noopener noreferrer" {
                                "@" (username)
                            }
                            " · "
                        }
                        (country_name) " · " span class="whitespace-nowrap" { "exported " (format_timestamp(watchlist.uploaded_at)) }
                    }
                    @if age_days >= STALE_AFTER_DAYS {
                        p class="mt-1 text-xs text-amber-400" {
                            "This export is " (age_days) " days old and won't include films you've added since."
                        }
                    }
                }
                a class="flex-shrink-0 mt-1 rounded-md border border-slate-600 px-2.5 py-1 text-xs font-medium text-slate-300 hover:border-orange-500 hover:text-orange-400" href="/" { "New export" }
            }

            @if tabs.is_empty() {
                p class="mt-6 rounded-lg border border-slate-700 bg-slate-800 p-4 text-slate-400" { "No recent films found in your watchlist." }
            } @else {
                style { (Raw::dangerously_create(format!(".tab-panel{{display:none}}{tab_css}"))) }
                div class="mt-4" {
                    @for (i, (id, ..)) in tabs.iter().enumerate() {
                        input type="radio" name="tab" class="sr-only" id=(format!("tab-{id}")) checked[i == 0];
                    }
                    nav class="sticky top-0 z-20 -mx-3 px-3 py-2 sm:-mx-6 sm:px-6 bg-slate-900/90 backdrop-blur" {
                        div class="flex gap-1 rounded-lg bg-slate-800 p-1 border border-slate-700" {
                            @for (id, label, count) in &tabs {
                                label for=(format!("tab-{id}")) class="flex-1 cursor-pointer select-none rounded-md px-2 py-1.5 text-center text-sm font-medium text-slate-400 hover:text-slate-200" {
                                    (label)
                                    span class="ml-1.5 text-xs text-slate-500" { (count) }
                                }
                            }
                        }
                    }
                    div class="tab-panels mt-2" {
                        div class="tab-panel panel-upcoming" {
                            (month_groups(&upcoming, today, country))
                        }
                        div class="tab-panel panel-released" {
                            (month_groups(&released, today, country))
                        }
                        div class="tab-panel panel-no-dates" {
                            @if !no_dates.is_empty() {
                                p class="mt-2 mb-2 text-sm text-slate-400" { "No cinema or digital release dates on TMDB yet." }
                                (title_list(no_dates.iter().map(|f| (f.title.as_str(), f.year, f.letterboxd_uri.as_str()))))
                            }
                            @if !unmatched.is_empty() {
                                h2 class="mt-6 mb-1 text-sm font-semibold text-slate-200" { "Not found on TMDB" }
                                p class="mb-2 text-sm text-slate-400" { "No film on TMDB has exactly this title and year." }
                                (title_list(unmatched.iter().map(|f| (f.title.as_str(), f.year, f.letterboxd_uri.as_str()))))
                            }
                        }
                    }
                }
            }

            footer class="mt-10 space-y-1 border-t border-slate-800 pt-4 text-xs text-slate-500" {
                @if uses_fallback {
                    p {
                        "Where " (country_name) " has no dates yet, " (fallback) " dates are shown and tagged with the country."
                    }
                }
                p { "Bookmark this page to come back without uploading again." }
                p {
                    "Release dates and posters from "
                    a class="text-slate-400 hover:text-orange-400" href="https://www.themoviedb.org/" target="_blank" rel="noopener noreferrer" { "TMDB" }
                    ", streaming availability from JustWatch."
                }
            }
        }
    })
}

fn month_groups<'a>(
    films: &'a [(Date, &'a FilmWithReleases)],
    today: Date,
    country: &'a str,
) -> impl Renderable + 'a {
    maud! {
        @for group in films.chunk_by(|(a, _), (b, _)| (a.year(), a.month()) == (b.year(), b.month())) {
            section {
                h2 class="mt-4 mb-2 text-xs font-semibold uppercase tracking-wider text-slate-500" {
                    (group[0].0.strftime("%B %Y").to_string())
                }
                div class="space-y-2" {
                    @for (_, film) in group {
                        (film_card(film, today, country))
                    }
                }
            }
        }
    }
}

fn title_list<'a>(
    films: impl Iterator<Item = (&'a str, Option<i16>, &'a str)> + 'a,
) -> impl Renderable + 'a {
    let films: Vec<_> = films.collect();
    maud! {
        ul class="rounded-lg border border-slate-700 bg-slate-800 divide-y divide-slate-700" {
            @for (title, year, uri) in &films {
                li class="px-3 py-2 text-sm" {
                    a class="text-slate-200 hover:text-orange-400" href=(uri) target="_blank" rel="noopener noreferrer" {
                        (title)
                        @if let Some(year) = year {
                            span class="ml-1.5 text-slate-500" { (year) }
                        }
                    }
                }
            }
        }
    }
}

pub fn error_fragment(message: String) -> String {
    content_div(maud! {
        div class="max-w-2xl mx-auto px-3 py-12 sm:px-6" {
            div class="bg-slate-800 shadow-xl rounded-lg p-6 sm:p-8 border border-slate-700" {
                h1 class="text-2xl font-bold text-slate-100" { "Error" }
                p class="mt-4 text-slate-400" { (message) }
                a class="mt-6 inline-block text-orange-500 hover:text-orange-400" href="/" { "Back" }
            }
        }
    })
}

pub fn error_page(message: String) -> String {
    page(
        "Error",
        maud! {
            div class="min-h-screen bg-slate-900 flex items-center justify-center" {
                div class="max-w-xl w-full px-6" {
                    div class="bg-slate-800 shadow-xl rounded-lg p-8 border border-slate-700" {
                        h1 class="text-2xl font-bold text-slate-100" { "Error" }
                        p class="mt-4 text-slate-400" { (message) }
                        a class="mt-6 inline-block text-orange-500 hover:text-orange-400" href="/" { "Back" }
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
            body class="bg-slate-900" { (body) }
        }
    }
    .render()
    .into_inner()
}

fn content_div(inner: impl Renderable) -> String {
    maud! { div id="content" class="min-h-screen bg-slate-900" { (inner) } }.render().into_inner()
}

fn film_card<'a>(
    film: &'a FilmWithReleases,
    today: Date,
    country: &'a str,
) -> impl Renderable + 'a {
    maud! {
        article class="flex gap-3 rounded-lg border border-slate-700/70 bg-slate-800 p-2.5" {
            a class="block flex-shrink-0" href=(&film.letterboxd_uri) target="_blank" rel="noopener noreferrer" tabindex="-1" {
                @if let Some(poster_path) = &film.poster_path {
                    img
                        class="w-12 h-[72px] sm:w-14 sm:h-[84px] object-cover rounded"
                        src=(format!("https://image.tmdb.org/t/p/w154{}", poster_path))
                        alt=""
                        loading="lazy"
                        width="64"
                        height="96";
                } @else {
                    div class="w-12 h-[72px] sm:w-14 sm:h-[84px] rounded bg-slate-700" {}
                }
            }
            div class="flex-1 min-w-0" {
                h3 class="font-semibold leading-snug" {
                    a class="text-slate-100 hover:text-orange-400" href=(&film.letterboxd_uri) target="_blank" rel="noopener noreferrer" {
                        (film.title)
                    }
                    @if let Some(year) = film.year {
                        span class="ml-1.5 font-normal text-slate-500" { (year) }
                    }
                }
                div class="mt-1.5 space-y-1" {
                    (release_row("Cinemas", &film.theatrical, ReleaseType::Theatrical, today, country))
                    (release_row("Streaming", &film.streaming, ReleaseType::Digital, today, country))
                }
                @if !film.streaming_providers.is_empty() {
                    (provider_list(&film.streaming_providers))
                }
            }
        }
    }
}

fn provider_list(providers: &[WatchProvider]) -> impl Renderable + '_ {
    let groups = [
        ("Stream", ProviderType::Stream),
        ("Rent", ProviderType::Rent),
        ("Buy", ProviderType::Buy),
    ]
    .map(|(label, kind)| {
        (label, providers.iter().filter(move |p| p.provider_type == kind).collect::<Vec<_>>())
    });

    maud! {
        div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1.5" {
            @for (label, group) in &groups {
                @if !group.is_empty() {
                    div class="flex items-center gap-1.5" {
                        span class="text-xs text-slate-500" { (label) }
                        @for provider in group {
                            (provider_icon(provider))
                        }
                    }
                }
            }
        }
    }
}

fn provider_icon(provider: &WatchProvider) -> impl Renderable + '_ {
    let icon = maud! {
        img
            class="w-6 h-6 rounded"
            src=(format!("https://image.tmdb.org/t/p/w92{}", provider.logo_path))
            alt=(provider.provider_name)
            loading="lazy"
            width="24"
            height="24";
    };

    maud! {
        @if let Some(link) = &provider.link {
            a href=(link) target="_blank" rel="noopener noreferrer" title=(provider.provider_name) class="block" { (icon) }
        } @else {
            span title=(provider.provider_name) class="block" { (icon) }
        }
    }
}

fn release_row<'a>(
    label: &'a str,
    releases: &'a [ReleaseDate],
    kind: ReleaseType,
    today: Date,
    country: &'a str,
) -> impl Renderable + 'a {
    let (badge, icon) = match kind {
        ReleaseType::Theatrical => (
            "bg-purple-500/15 text-purple-300 ring-purple-400/30",
            "M16.5 6v.75m0 3v.75m0 3v.75m0 3V18m-9-5.25h5.25M7.5 15h3M3.375 5.25c-.621 \
             0-1.125.504-1.125 1.125v3.026a2.999 2.999 0 0 1 0 5.198v3.026c0 .621.504 1.125 1.125 \
             1.125h17.25c.621 0 1.125-.504 1.125-1.125v-3.026a2.999 2.999 0 0 1 \
             0-5.198V6.375c0-.621-.504-1.125-1.125-1.125H3.375Z",
        ),
        ReleaseType::Digital => (
            "bg-sky-500/15 text-sky-300 ring-sky-400/30",
            "M6 20.25h12m-7.5-3v3m3-3v3m-10.125-3h17.25c.621 0 1.125-.504 \
             1.125-1.125V4.875c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 \
             1.125v11.25c0 .621.504 1.125 1.125 1.125Z",
        ),
    };
    let mut releases: Vec<_> = releases.iter().collect();
    releases.sort_by_key(|r| r.date);
    let past = releases.first().map(|r| r.date < today);
    let fallback = releases.iter().find_map(|r| r.note.as_deref().filter(|n| *n != country));

    maud! {
        @if let Some(past) = past {
            div class="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-sm" {
                span class=(format!("inline-flex flex-shrink-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-xs font-medium ring-1 ring-inset {badge}")) {
                    (Raw::dangerously_create(format!(r#"<svg class="h-3.5 w-3.5 flex-shrink-0" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.75" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="{icon}"/></svg>"#)))
                    (label)
                }
                span class=(if past { "text-slate-400" } else { "font-medium text-slate-100" }) {
                    (releases.iter().map(|r| r.date.strftime("%-d %b %Y").to_string()).collect::<Vec<_>>().join(", "))
                }
                @if let Some(code) = fallback {
                    span class="rounded border border-slate-600 px-1 text-[10px] font-medium leading-4 text-slate-400" title=(format!("{} release date", get_country_name(code))) { (code) }
                }
            }
        }
    }
}

fn country_selector_script() -> impl Renderable {
    maud! {
        script {
            (Raw::dangerously_create(r#"
                let selectedIndex = -1;

                function selectCountry(code, name) {
                    document.getElementById('country').value = code;
                    document.getElementById('country-search').value = name;
                    document.getElementById('country-dropdown').classList.add('hidden');
                    selectedIndex = -1;
                    validateForm();
                    document.getElementById('submit-button').focus();
                }

                function isFormReady() {
                    const country = document.getElementById('country').value.trim();
                    return Boolean(country) && document.getElementById('file').files.length > 0;
                }

                function validateForm() {
                    document.getElementById('submit-button').disabled = !isFormReady();
                }

                function submitUpload() {
                    if (isFormReady()) {
                        startUpload();
                        document.getElementById('upload-form').submit();
                    }
                }

                function startUpload() {
                    const submitButton = document.getElementById('submit-button');
                    submitButton.disabled = true;
                    submitButton.textContent = 'Uploading…';
                }

                function setDragging(dragging) {
                    const zone = document.getElementById('drop-zone');
                    zone.classList.toggle('border-orange-500', dragging);
                    zone.classList.toggle('bg-slate-700', dragging);
                }

                function fileChosen() {
                    const file = document.getElementById('file').files[0];
                    document.getElementById('file-name').textContent =
                        file ? file.name : 'Drop the ZIP here, or click to choose it';
                    validateForm();
                    submitUpload();
                }

                function handleCountryKeydown(event) {
                    const dropdown = document.getElementById('country-dropdown');
                    const isOpen = !dropdown.classList.contains('hidden');

                    if (event.key === 'Enter' && !isOpen) {
                        event.preventDefault();
                        submitUpload();
                    }
                }

                function getVisibleOptions() {
                    const dropdown = document.getElementById('country-dropdown');
                    const options = dropdown.getElementsByClassName('country-option');
                    const visible = [];
                    for (let i = 0; i < options.length; i++) {
                        if (options[i].style.display !== 'none') {
                            visible.push(options[i]);
                        }
                    }
                    return visible;
                }

                function highlightOption(index) {
                    const visible = getVisibleOptions();
                    visible.forEach((opt, i) => {
                        if (i === index) {
                            opt.classList.add('bg-blue-100');
                            opt.scrollIntoView({ block: 'nearest' });
                        } else {
                            opt.classList.remove('bg-blue-100');
                        }
                    });
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

                    selectedIndex = -1;
                    if (hasVisible) {
                        dropdown.classList.remove('hidden');
                    }
                }

                const searchInput = document.getElementById('country-search');
                const dropdown = document.getElementById('country-dropdown');
                
                function focusOption(index) {
                    const visible = getVisibleOptions();
                    if (index >= 0 && index < visible.length) {
                        visible[index].focus();
                    }
                }
                
                searchInput.addEventListener('keydown', function(e) {
                    const isOpen = !dropdown.classList.contains('hidden');
                    const visible = getVisibleOptions();
                    
                    switch(e.key) {
                        case 'ArrowDown':
                            e.preventDefault();
                            if (!isOpen) {
                                dropdown.classList.remove('hidden');
                            }
                            if (visible.length > 0) {
                                selectedIndex = selectedIndex < 0 ? 0 : (selectedIndex + 1) % visible.length;
                                highlightOption(selectedIndex);
                                focusOption(selectedIndex);
                            }
                            break;
                            
                        case 'ArrowUp':
                            e.preventDefault();
                            if (!isOpen) {
                                dropdown.classList.remove('hidden');
                            }
                            if (visible.length > 0) {
                                selectedIndex = selectedIndex <= 0 ? visible.length - 1 : selectedIndex - 1;
                                highlightOption(selectedIndex);
                                focusOption(selectedIndex);
                            }
                            break;
                            
                        case 'Enter':
                            if (isOpen) {
                                e.preventDefault();
                                if (selectedIndex >= 0 && selectedIndex < visible.length) {
                                    const option = visible[selectedIndex];
                                    selectCountry(option.getAttribute('data-code'), option.getAttribute('data-name'));
                                }
                            }
                            break;
                            
                        case ' ':
                            if (isOpen && selectedIndex >= 0) {
                                e.preventDefault();
                                if (selectedIndex < visible.length) {
                                    const option = visible[selectedIndex];
                                    selectCountry(option.getAttribute('data-code'), option.getAttribute('data-name'));
                                }
                            }
                            break;
                            
                        case 'Escape':
                            if (isOpen) {
                                e.preventDefault();
                                dropdown.classList.add('hidden');
                                selectedIndex = -1;
                                searchInput.focus();
                            }
                            break;
                    }
                });
                
                dropdown.addEventListener('keydown', function(e) {
                    const visible = getVisibleOptions();
                    const focusedElement = document.activeElement;
                    const currentIndex = visible.indexOf(focusedElement);
                    
                    switch(e.key) {
                        case 'ArrowDown':
                            e.preventDefault();
                            if (visible.length > 0) {
                                selectedIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % visible.length;
                                highlightOption(selectedIndex);
                                focusOption(selectedIndex);
                            }
                            break;
                            
                        case 'ArrowUp':
                            e.preventDefault();
                            if (visible.length > 0) {
                                selectedIndex = currentIndex <= 0 ? visible.length - 1 : selectedIndex - 1;
                                highlightOption(selectedIndex);
                                focusOption(selectedIndex);
                            }
                            break;
                            
                        case 'Enter':
                        case ' ':
                            e.preventDefault();
                            if (focusedElement.classList.contains('country-option')) {
                                selectCountry(focusedElement.getAttribute('data-code'), focusedElement.getAttribute('data-name'));
                            }
                            break;
                            
                        case 'Escape':
                            e.preventDefault();
                            dropdown.classList.add('hidden');
                            selectedIndex = -1;
                            searchInput.focus();
                            break;
                    }
                });

                document.addEventListener('click', function(event) {
                    if (dropdown && searchInput && !dropdown.contains(event.target) && event.target !== searchInput) {
                        dropdown.classList.add('hidden');
                        selectedIndex = -1;
                    }
                });

                // With no saved or IP-based country, guess from the timezone, then the language.
                function suggestCountry() {
                    if (document.getElementById('country').value) {
                        return;
                    }
                    const options = Array.from(document.getElementsByClassName('country-option'));
                    const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
                    const locales = (navigator.languages || [navigator.language]).flatMap(tag => {
                        try {
                            return [new Intl.Locale(tag)];
                        } catch {
                            return [];
                        }
                    });
                    const regions = [
                        ...locales.map(locale => locale.region),
                        ...locales.map(locale => locale.maximize().region),
                    ];
                    const option =
                        options.find(o => o.getAttribute('data-timezones').split(' ').includes(timeZone)) ||
                        regions.map(r => options.find(o => o.getAttribute('data-code') === r)).find(Boolean);
                    if (option) {
                        document.getElementById('country').value = option.getAttribute('data-code');
                        document.getElementById('country-search').value = option.getAttribute('data-name');
                    }
                }
                suggestCountry();

                // Also runs when the back button restores the page after an upload
                window.addEventListener('pageshow', function() {
                    document.getElementById('submit-button').textContent = 'Find release dates';
                    validateForm();
                });
            "#))
        }
    }
}
