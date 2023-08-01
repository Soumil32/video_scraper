use scraper::{Html, Selector};
use std::error::Error;
use headless_chrome::Browser;
//use headless_chrome::protocol::cdp::Page;

fn main() -> Result<(), Box<dyn Error>> {
    // The URL you want to scrape
    let url = "https://www.youtube.com/watch?v=UDVNv5Ux-fU"; // Replace with the target URL

    let browser = Browser::default()?;
    let tab = browser.new_tab()?;
    tab.navigate_to(url)?;
    tab.wait_until_navigated()?;
    let body = tab.get_content().unwrap();

    // Parse the HTML using scraper
    let fragment = Html::parse_document(&body);

    // Define the selector for video links
    let video_selector = Selector::parse("video").unwrap();

    // Iterate through the links and print them
    for link in fragment.select(&video_selector) {
        let src = link.value().attr("src").unwrap();
        if src.contains("blob:") {
            println!("The link to download the video is: {}", src)
        } else {
            println!("The link to download the video is: {}", src);
        }
    }

    Ok(())
}

