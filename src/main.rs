use std::{collections::HashMap, env, error::Error, fs::{self, File, OpenOptions}, io::Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let videos = fs::read_to_string("videos.txt").unwrap();
    let videos = videos.split("\n");
    
    for video in videos {
        if video.len() > 0 {
            if video.starts_with("#") {
                continue;
            }
            let video = video.split("|").collect::<Vec<&str>>();
            let url = video[0];
            let file_path = video[1];
            println!("Downloading {} to {}", url, file_path);
            download_video(url.to_string(), file_path.to_string()).await;
        }
    }
    Ok(())
}

async fn download_video(url: String, file_path: String) {

    let res = reqwest::get(url).await.unwrap(); // Send a GET request and wait for the response
    let body = res.text().await.unwrap(); // Read the response body as text
    let mut file = File::create("master.m3u8").unwrap(); // Create a file to write to
    file.write_all(body.as_bytes()).unwrap(); // Write the body to the file

    let (_, master_m3u8) = m3u8_rs::parse_master_playlist(&body.as_bytes()).unwrap();

    let media_m3u8_url = &master_m3u8
        .variants
        .iter()
        .find(|item| item.resolution.unwrap().height == 720)
        .unwrap()
        .uri;
    let res = reqwest::get(media_m3u8_url).await.unwrap(); // Send a GET request and wait for the response
    let body = res.text().await.unwrap(); // Read the response body as text
    let mut file = File::create("index.m3u8").unwrap(); // Create a file to write to
    file.write_all(body.as_bytes()).unwrap(); // Write the body to the file

    let (_, index_m3u8) = m3u8_rs::parse_media_playlist(&body.as_bytes()).unwrap();
    let video_base_url = {
        // remove everything after the last slash
        let mut url = media_m3u8_url.clone();
        url.truncate(url.rfind('/').unwrap());
        url
    };
    println!("Video base url: {}", video_base_url);
    
    let mut tasks = Vec::new();
    for (i, segment) in index_m3u8.segments.iter().enumerate() {
        let video_url = video_base_url.clone() + "/" + &segment.uri;
        let future = tokio::spawn(download_segment(video_url, i as u16));
        tasks.push(future);
    }

    let mut results = HashMap::with_capacity(tasks.len());
    for task in tasks {
        let (order, segment) = task.await.unwrap();
        results.insert(order, segment);
    }

    let mut video_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path + ".ts")
        .unwrap();
    println!("{}", results.len() == index_m3u8.segments.len());
    let mut buffer = Vec::new();
    for i in 0..results.len() {
        buffer.append(results.entry(i as u16).or_default());
    }
    match video_file.write_all(&buffer) {
        Ok(_) => println!("Video downloaded"),
        Err(e) => println!("Error writing to file: {}", e)
    }
}

async fn download_segment(url: String, order: u16) -> (u16, Vec<u8>) {
    let reqwest_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(100000))
        .build()
        .unwrap();
    // Order is used to keep track of the order of the segments
    // since they can be downloaded in any order
    // but have to be written to the file in the correct order
    let segment = reqwest_client.get(&url).send().await;
    let mut bytes = Vec::new();
    
    match segment {
        Ok(segment) => {
            if segment.status().is_success() {
                bytes = segment.bytes().await.unwrap().to_vec();
            } else {
                loop {
                    let segment_res = reqwest_client.get(&url).send().await;
                
                    if let Ok(segment) = segment_res {
                        if !segment.status().is_success() {
                            continue;
                        }
                        bytes = segment.bytes().await.unwrap().to_vec();
                        break;
                    }
                }
            }
        }
        Err(e) => {
            if e.is_redirect() {
                let redirect_url = e.url().unwrap().to_string();
                println!("Redirected to {}", redirect_url);
                let segment = reqwest_client.get(&redirect_url).send().await;
                if let Err(e) = segment {
                    panic!("Error downloading segment: {}", e);
                } else {
                    bytes = segment.unwrap().bytes().await.unwrap().to_vec();
                }
            } else if e.is_timeout() {
                println!("Timeout, retrying");
                loop{
                    let segment = reqwest_client.get(&url).send().await;
                
                    
                    if segment.is_ok() {
                        bytes = segment.unwrap().bytes().await.unwrap().to_vec();
                        break;
                    }
                }
            } else if e.is_connect() {
                println!("Could not connect, retrying");
                loop {
                    let segment = reqwest_client.get(&url).send().await;
                    if let Err(e) = segment {
                        panic!("Error downloading segment, could not connect: {}", e);
                    } else {
                        bytes = segment.unwrap().bytes().await.unwrap().to_vec();
                        break;
                    }
                }
            } else {
                panic!("Error downloading segment for unknown reason: {}", e);
            }
        }
    }

    println!("Segment downloaded {}", order);
    (order, bytes)
}