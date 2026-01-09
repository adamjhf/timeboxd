use hypertext::{Raw, maud, prelude::*};

use crate::{
    countries::{COUNTRIES, get_country_name},
    models::{
        FilmWithReleases, ProviderType, ReleaseCategory, ReleaseDate, ReleaseType, WatchProvider,
    },
};

const TAILWIND_CDN: &str = "https://cdn.tailwindcss.com";
const DATASTAR_CDN: &str =
    "https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.9/dist/datastar.js";

pub fn index_page(saved_username: Option<&str>, saved_country: Option<&str>) -> String {
    let country_name = saved_country.map(get_country_name);

    page(
        "Timeboxd - upcoming film releases from your Letterboxd watchlist",
        maud! {
            div class="min-h-screen bg-slate-900" {
                div class="max-w-2xl mx-auto px-6 py-12" {
                    div class="bg-slate-800 shadow-xl rounded-lg p-8 border border-slate-700" {
                        h1 class="text-3xl font-bold text-slate-100" { "Timeboxd" }
                        p class="mt-2 text-slate-400" { "Upcoming film release dates for your Letterboxd watchlist." }

                        form class="mt-8 space-y-6" method="get" action="/release-dates" {
                            div {
                                label class="block text-sm font-medium text-slate-300" for="username" { "Letterboxd username" }
                                input
                                    class="mt-2 w-full rounded-md border border-slate-600 bg-slate-700 text-slate-100 px-3 py-2 placeholder-slate-400 focus:border-orange-500 focus:outline-none focus:ring-1 focus:ring-orange-500"
                                    name="username"
                                    id="username"
                                    value=[saved_username]
                                    required;
                            }

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
                                        onfocus="document.getElementById('country-dropdown').classList.remove('hidden')"
                                        ;
                                    input type="hidden" name="country" id="country" value=[saved_country] required;
                                    div id="country-dropdown" class="hidden absolute z-10 mt-1 w-full bg-slate-700 border border-slate-600 rounded-md shadow-lg max-h-60 overflow-y-auto" {
                                        @for country in COUNTRIES {
                                            div
                                                class="country-option px-3 py-2 text-slate-200 hover:bg-slate-600 cursor-pointer focus:bg-orange-900 focus:outline-none"
                                                data-code=(country.code)
                                                data-name=(country.name)
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

                             button id="submit-button" class="w-full rounded-md bg-orange-600 px-4 py-2 font-semibold text-white hover:bg-orange-700 focus:outline-none focus:ring-1 focus:ring-orange-500" type="submit" { "Find release dates" }
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
        &format!("Upcoming film releases for {username} - Timeboxd"),
        maud! {
            div class="min-h-screen bg-slate-900 flex items-center justify-center" {
                div id="content" class="max-w-xl w-full px-6" {
                    div class="bg-slate-800 shadow-xl rounded-lg p-8 text-center border border-slate-700" {
                        div class="mx-auto h-12 w-12 rounded-full border-4 border-slate-700 border-t-orange-600 animate-spin" {}
                        h1 class="mt-6 text-xl font-semibold text-slate-100" { "Processing" }
                        p class="mt-2 text-slate-400" { "Fetching watchlist and checking release dates." }
                        p class="mt-2 text-sm text-slate-500" { "This may take a minute for large watchlists." }
                    }
                }
            }
            script { (Raw::dangerously_create(format!("
                fetch('{}')
                    .then(response => response.text())
                    .then(html => {{
                        document.getElementById('content').innerHTML = html;
                        document.title = 'Upcoming film releases for {} - Timeboxd';
                    }})
                    .catch(error => {{
                        document.getElementById('content').innerHTML = '<div class=\"bg-slate-800 shadow-xl rounded-lg p-8 border border-slate-700\"><h1 class=\"text-2xl font-bold text-slate-100\">Error</h1><p class=\"mt-4 text-slate-400\">' + error.message + '</p></div>';
                    }});
            ", url, username))) }
        },
    )
}

pub fn results_fragment(username: &str, country: &str, films: &[FilmWithReleases]) -> String {
    let country_name = get_country_name(country);
    let letterboxd_user_url = format!("https://letterboxd.com/{}/", username);

    let today: jiff::civil::Date = jiff::Zoned::now().into();
    let current_year = today.year();
    let min_year = current_year - 1;

    fn sort_by_first_release_date(films: &mut Vec<&FilmWithReleases>) {
        films.sort_by(|a, b| {
            let a_first_date = a.theatrical.first().or_else(|| a.streaming.first()).map(|r| r.date);
            let b_first_date = b.theatrical.first().or_else(|| b.streaming.first()).map(|r| r.date);

            match (a_first_date, b_first_date) {
                (Some(ad), Some(bd)) => ad.cmp(&bd).then(a.title.cmp(&b.title)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.title.cmp(&b.title),
            }
        });
    }

    fn sort_by_release_date(films: &mut Vec<&FilmWithReleases>) {
        films.sort_by(|a, b| {
            let a_date = a.theatrical.first().or_else(|| a.streaming.first()).map(|r| r.date);
            let b_date = b.theatrical.first().or_else(|| b.streaming.first()).map(|r| r.date);

            match (a_date, b_date) {
                (Some(ad), Some(bd)) => ad.cmp(&bd).then(a.title.cmp(&b.title)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.title.cmp(&b.title),
            }
        });
    }

    fn sort_by_year(films: &mut Vec<&FilmWithReleases>) {
        films.sort_by(|a, b| match (a.year, b.year) {
            (Some(ay), Some(by)) => ay.cmp(&by).then(a.title.cmp(&b.title)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.title.cmp(&b.title),
        });
    }

    let mut local_upcoming_films: Vec<_> = films
        .iter()
        .filter(|f| f.category == ReleaseCategory::LocalUpcoming)
        .filter(|f| f.year.is_some_and(|y| y >= min_year))
        .collect();
    let mut local_already_available_films: Vec<_> = films
        .iter()
        .filter(|f| f.category == ReleaseCategory::LocalAlreadyAvailable)
        .filter(|f| f.year.is_some_and(|y| y >= min_year))
        .collect();
    let mut no_releases: Vec<_> = films
        .iter()
        .filter(|f| f.category == ReleaseCategory::NoReleases)
        .filter(|f| f.year.map_or(true, |y| y >= min_year))
        .collect();

    sort_by_first_release_date(&mut local_upcoming_films);
    sort_by_release_date(&mut local_already_available_films);
    sort_by_year(&mut no_releases);

    content_div(maud! {
        div class="max-w-4xl mx-auto px-4 py-4" {
             div class="flex items-start justify-between gap-4" {
                 div {
                     h1 class="text-2xl font-bold text-slate-100" { "Timeboxd" }
                     p class="mt-1 text-sm text-slate-400" { "Local release dates for your Letterboxd watchlist" }
                     p class="mt-1 text-sm text-slate-400" {
                         a class="text-orange-500 hover:text-orange-400" href=(letterboxd_user_url) target="_blank" rel="noopener noreferrer" {
                             "@" (username)
                         }
                         " · " (country_name)
                     }
                 }
                a class="text-sm text-orange-500 hover:text-orange-400" href="/" { "New query" }
            }

            @if films.is_empty() {
                div class="mt-4 bg-slate-800 shadow-xl rounded-lg p-4 border border-slate-700" {
                    p class="text-slate-400" { "No films found in watchlist." }
                }
            } @else {
                @if !local_upcoming_films.is_empty() {
                    div class="mt-4" {
                        h2 class="text-lg font-semibold text-slate-200 mb-2" { "Upcoming releases" }
                        @if country == "NZ" {
                            p class="text-sm text-slate-400 mb-2" { "Falls back to Australia then US release dates if no local dates found" }
                        } @else {
                            p class="text-sm text-slate-400 mb-2" { "Falls back to US release dates if no local dates found" }
                        }
                        div class="space-y-2" {
                            @for film in &local_upcoming_films {
                                (film_card(film))
                            }
                        }
                    }
                }



                @if !local_already_available_films.is_empty() {
                    div class="mt-6" {
                        h2 class="text-lg font-semibold text-slate-200 mb-2" { "Recent releases" }
                        p class="text-sm text-slate-400 mb-2" { "Films released in the last year" }
                        @if country == "NZ" {
                            p class="text-sm text-slate-400 mb-2" { "Falls back to Australia then US release dates if no local dates found" }
                        } @else {
                            p class="text-sm text-slate-400 mb-2" { "Falls back to US release dates if no local dates found" }
                        }
                        div class="space-y-2" {
                            @for film in &local_already_available_films {
                                (film_card(film))
                            }
                        }
                    }
                }

                @if !no_releases.is_empty() {
                    div class="mt-6" {
                        h2 class="text-lg font-semibold text-slate-200 mb-2" { "No release dates found" }
                        div class="space-y-2" {
                            @for film in &no_releases {
                                (film_card(film))
                            }
                        }
                    }
                }

                @if local_upcoming_films.is_empty() && local_already_available_films.is_empty() && no_releases.is_empty() {
                    div class="mt-4 bg-slate-800 shadow-xl rounded-lg p-4 border border-slate-700" {
                        p class="text-slate-400" { "No films processed." }
                    }
                }
            }
        }
    })
}

pub fn error_fragment(message: String) -> String {
    content_div(maud! {
        div class="max-w-2xl mx-auto px-6 py-12" {
            div class="bg-slate-800 shadow-xl rounded-lg p-8 border border-slate-700" {
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
    let letterboxd_url = format!("https://letterboxd.com/film/{}/", film.letterboxd_slug);

    maud! {
        div class="bg-slate-800 shadow-xl rounded p-3 flex gap-3 border border-slate-700" {
            @if let Some(poster_path) = &film.poster_path {
                a
                    class="block flex-shrink-0 w-20"
                    href=(letterboxd_url.clone())
                    target="_blank"
                    rel="noopener noreferrer"
                {
                    img
                        class="w-20 h-30 object-cover rounded"
                        src=(format!("https://image.tmdb.org/t/p/w200{}", poster_path))
                        alt=(format!("{} poster", film.title))
                        loading="lazy"
                        width="80"
                        height="120";
                }
            } @else {
                div class="flex-shrink-0 w-20 h-30 bg-slate-700 rounded flex items-center justify-center border border-slate-600" {
                    span class="text-xs text-slate-500" { "No poster" }
                }
            }
            div class="flex-1 min-w-0" {
                div class="flex items-start justify-between gap-2" {
                    div class="flex-1 min-w-0" {
                        h2 class="text-lg font-semibold" {
                            a class="text-slate-100 hover:text-orange-500" href=(letterboxd_url) target="_blank" rel="noopener noreferrer" {
                                (film.title)
                                @if let Some(year) = film.year {
                                    span class="ml-1.5 font-normal text-slate-400" { "(" (year) ")" }
                                }
                            }
                        }
                        div class="mt-0.5 text-xs" {
                            a class="text-slate-500 hover:text-slate-400" href=(format!("https://www.themoviedb.org/movie/{}", film.tmdb_id)) target="_blank" rel="noopener noreferrer" {
                                "TMDB"
                            }
                        }
                    }
                }

                div class="mt-2 grid grid-cols-2 gap-3" {
                    (release_list("Theatrical", &film.theatrical, ReleaseType::Theatrical))
                    (release_list("Streaming", &film.streaming, ReleaseType::Digital))
                }

                @if !film.streaming_providers.is_empty() {
                    (provider_list(&film.streaming_providers))
                }
            }
        }
    }
}

fn provider_list(providers: &[WatchProvider]) -> impl Renderable + '_ {
    let stream_providers: Vec<_> =
        providers.iter().filter(|p| p.provider_type == ProviderType::Stream).collect();
    let rent_providers: Vec<_> =
        providers.iter().filter(|p| p.provider_type == ProviderType::Rent).collect();
    let buy_providers: Vec<_> =
        providers.iter().filter(|p| p.provider_type == ProviderType::Buy).collect();

    maud! {
        div class="mt-3 border-t border-slate-700 pt-3" {
            h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wide mb-2" { "Available now" }
            div class="space-y-2" {
                @if !stream_providers.is_empty() {
                    div class="flex items-center gap-2" {
                        span class="text-xs text-slate-500 w-12" { "Stream" }
                        div class="flex flex-wrap gap-1.5" {
                            @for provider in &stream_providers {
                                (provider_icon(provider))
                            }
                        }
                    }
                }
                @if !rent_providers.is_empty() {
                    div class="flex items-center gap-2" {
                        span class="text-xs text-slate-500 w-12" { "Rent" }
                        div class="flex flex-wrap gap-1.5" {
                            @for provider in &rent_providers {
                                (provider_icon(provider))
                            }
                        }
                    }
                }
                @if !buy_providers.is_empty() {
                    div class="flex items-center gap-2" {
                        span class="text-xs text-slate-500 w-12" { "Buy" }
                        div class="flex flex-wrap gap-1.5" {
                            @for provider in &buy_providers {
                                (provider_icon(provider))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn provider_icon(provider: &WatchProvider) -> impl Renderable + '_ {
    maud! {
        @if let Some(link) = &provider.link {
            a
                href=(link)
                target="_blank"
                rel="noopener noreferrer"
                title=(provider.provider_name)
                class="block"
            {
                img
                    class="w-7 h-7 rounded"
                    src=(format!("https://image.tmdb.org/t/p/w92{}", provider.logo_path))
                    alt=(provider.provider_name)
                    loading="lazy"
                    width="28"
                    height="28";
            }
        } @else {
            span title=(provider.provider_name) class="block" {
                img
                    class="w-7 h-7 rounded"
                    src=(format!("https://image.tmdb.org/t/p/w92{}", provider.logo_path))
                    alt=(provider.provider_name)
                    loading="lazy"
                    width="28"
                    height="28";
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
        ReleaseType::Theatrical => "border-purple-400",
        ReleaseType::Digital => "border-blue-400",
    };

    maud! {
        div class=(format!("border-l-3 {} pl-2.5", border)) {
            h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wide" { (label) }
            @if releases.is_empty() {
                p class="mt-1 text-sm text-slate-500" { "—" }
            } @else {
                ul class="mt-1 space-y-0.5" {
                    @for rel in releases {
                        li class="text-sm text-slate-300" {
                            span class="font-medium" { (format_date(rel)) }
                            @if let Some(note) = &rel.note {
                                span class="text-slate-500" { " · " (note) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_date(rel: &ReleaseDate) -> String {
    rel.date.strftime("%-d %b %Y").to_string()
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
                    document.getElementById('submit-button').focus();
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
                                selectedIndex = currentIndex <= 0 ? visible.length - 1 : currentIndex - 1;
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
            "#))
        }
    }
}
