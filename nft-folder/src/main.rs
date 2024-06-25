use crossbeam_channel::{bounded, unbounded, Sender};
use crossbeam_utils::thread::scope;
use ethers::core::rand::random;
use std::{thread, time::Duration};

enum QueryResult {
    Urls(Vec<String>),
    Finished,
}

fn query_api(sender: Sender<QueryResult>, mut cursor: i32) {
    if cursor == 0 {
        let _ = sender.send(QueryResult::Finished);
        drop(sender);
        return;
    }

    let urls = vec![
        "https://example.com/image1.jpg".to_owned(),
        "https://example.com/image2.jpg".to_owned(),
        "https://example.com/image3.jpg".to_owned(),
        "https://example.com/image4.jpg".to_owned(),
        "https://example.com/image5.jpg".to_owned(),
        "https://example.com/image6.jpg".to_owned(),
    ];
    let r: u64 = random::<u64>() / 50093603030000000;
    thread::sleep(Duration::from_millis(r));
    println!("sending result");
    let _ = sender.send(QueryResult::Urls(urls));

    cursor -= 1;
    query_api(sender, cursor);
}

fn download_image(url: String, t: i32) {
    let r: u64 = random::<u64>() / 1009360303000000;

    println!("Get image {} on t:{t}", url);
    thread::sleep(Duration::from_millis(r));
    println!("Downloaded {} on t:{t}", url);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = unbounded();
    let (task_sender, task_receiver) = unbounded();

    let query_thread = thread::spawn(move || {
        query_api(sender, 4);
        
    });

    scope(|s| {
        for t in 0..5 {
            let task_receiver = task_receiver.clone();
            s.spawn(move |_| {
                for url in task_receiver {
                    download_image(url, t);
                }
            });
        }

        for task in receiver {
            match task {
                QueryResult::Urls(task) => {
                    for url in task {
                        let _ = task_sender.send(url);
                    }
                }
                QueryResult::Finished => {
                    drop(task_sender); 
                    break;
                }
            }
        }
    })
    .unwrap();

    query_thread.join().unwrap();

    Ok(())
}
