use std::time::Duration;

fn main() -> cu::Result<()> {
    cu::init_print_options(cu::ColorLevel::Auto, cu::PrintLevel::Normal, None);
    // cu::set_thread_print_name("main");
    cu::print!("today's weather is {}", "good");
    cu::hint!("today's weather is {}", "ok");
    cu::info!("this is an info messagenmultilineaa 你好 sldkfjals🤖kdjflkasjdflkjasldkfjaklsdjflkjasldkfjlaksjdflkajsdklfjlaksjdfkljasldkfjlasldkjflaskdjflaksjdlfkajsldkfjkasjdlfkjaskldjflajsdlkfjlaskjdfklajsdf");
    cu::warn!("this is a warn message\n");
    cu::error!("this is error message\n");
    cu::debug!("this is debug message\n");
    cu::trace!("this is trace message\n");
    if !cu::yesno!("continue?")? {
        cu::warn!("you chose to not continue!");
        return Ok(())
    }
    cu::info!("you chose to continue!");
    
    
    {
        let bar2 = cu::progress_bar(20, "This takes 5 seconds");
        let bar = cu::progress_bar(10, "This takes 2.5 seconds asldkfhalsdkhflaksdhflkashdlfkhaskldhfklashdfjklhaskljdhfajkshdkfljhasjkdhfklajshdfkjlhaskljdhfkajlshdalsdkhfalskdhflakshdflkhasldkfhlakshdflkashdlfkhalskdhflkashdfhf");
        for i in 0..10 {
            cu::progress!(bar, i, "step {i}");
            cu::progress!(bar2, i, "step {i}");
    cu::debug!("this is debug message\n");
            std::thread::sleep(Duration::from_millis(250));
        }
        drop(bar);
        for i in 0..10 {
            cu::progress!(bar2, i+10);
            std::thread::sleep(Duration::from_millis(250));
            cu::print!("doing stuff");
        }
    }
    
    let thread1 = std::thread::spawn(|| {
        cu::set_thread_print_name("t1");
        let answer = cu::prompt!("from thread 1").unwrap();
        cu::info!("you entered: {answer}");
    });
    let thread2 = std::thread::spawn(|| {
        cu::set_thread_print_name("t2");
        let answer = cu::prompt!("from thread 2").unwrap();
        cu::info!("you entered: {answer}");
    });
    let thread3 = std::thread::spawn(|| {
        cu::set_thread_print_name("t3");
        let answer = cu::prompt!("from thread 3").unwrap();
        cu::info!("you entered: {answer}");
    });
    let _ = thread1.join();
    let _ = thread2.join();
    let _ = thread3.join();
    cu::info!("all threads joined ok");
    
    Ok(())
}
