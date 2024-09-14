use std::error::Error;
use std::process::Command;
use base64::Engine;
use ollama_rs::{
    generation::{
        completion::{request::GenerationRequest, GenerationResponse},
        images::Image,
    },
    Ollama,
};
use reqwest::get;
use tokio::{runtime::Runtime,io::{stdout,AsyncWriteExt}};
use ffmpeg_next as ffmpeg;
use tokio_stream::StreamExt;
use image::{ImageBuffer, Rgb,ImageFormat};
use tempfile::NamedTempFile;



const IMAGE_URL: &str = "https://images.pexels.com/photos/1054655/pexels-photo-1054655.jpeg";
const PROMPT: &str = "This is image from an instagram reel, describe these images please.";
const MODEL: &str = "llava:latest";
const VIDEO_PATH :&str = "/Users/saurabhkaul/Downloads/Video-635.mov";
const BATCH_SIZE: usize = 5;
const FRAME_INTERVAL: usize = 120;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    ffmpeg::init()?;

    // Open the video file
    let mut ictx = ffmpeg::format::input(VIDEO_PATH)?;
    let input = ictx.streams().best(ffmpeg::media::Type::Video).ok_or("No video stream found")?;
    let video_stream_index = input.index();

    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    // Initialize Ollama client
    let ollama = Ollama::default();

    let mut frame_count = 0;
    let mut batch = Vec::new();

    let mut receive_and_process_frames = |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
        let mut decoded = ffmpeg::util::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            frame_count += 1;

            if frame_count % FRAME_INTERVAL == 0 {
                println!("Processing frame {}", frame_count);

                let width = decoded.width() as usize;
                let height = decoded.height() as usize;
                let stride = decoded.stride(0);
                let data = decoded.data(0);

                let mut buffer = ImageBuffer::new(width as u32, height as u32);

                for y in 0..height {
                    for x in 0..width {
                        let i = y * stride + x * 3;
                        if i + 2 < data.len() {
                            let r = data[i];
                            let g = data[i + 1];
                            let b = data[i + 2];
                            buffer.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
                        }
                    }
                }

                batch.push(buffer);

                // If we've reached the batch size, process the batch
                if batch.len() >= BATCH_SIZE {
                    match process_batch(&ollama, &batch){
                        Ok(_) => {batch.clear()}
                        Err(e) => {eprintln!("{}", e)}
                    }

                }
            }
        }
        Ok(())
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            receive_and_process_frames(&mut decoder)?;
        }
    }
    decoder.send_eof()?;
    receive_and_process_frames(&mut decoder)?;

    // Process any remaining frames in the batch
    if !batch.is_empty() {
        process_batch(&ollama, &batch)?;
    }

    println!("Processed {} frames", frame_count);
    Ok(())
}

fn process_batch(ollama: &Ollama, batch: &[ImageBuffer<Rgb<u8>, Vec<u8>>]) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing batch of {} frames", batch.len());

    // Here you would typically combine the frames into a single input for Ollama
    // For this example, we'll just concatenate descriptions of each frame
    // let mut combined_input = String::new();
    // for (i, frame) in batch.iter().enumerate() {
    //     combined_input.push_str(&format!("Frame {}: {}x{} image. ", i + 1, frame.width(), frame.height()));
    // }

    let mut images = Vec::new();
    for (i, frame) in batch.iter().enumerate() {
        let p = format!("output_frame_{i}.png");
        save_image_buffer(frame,&p);

        let mut png_image = Vec::new();
        frame.write_to(&mut std::io::Cursor::new(&mut png_image), ImageFormat::Png)?;
        let base64_image = base64::engine::general_purpose::STANDARD.encode(&png_image);
        images.push(Image::from_base64(&base64_image));
    }

    let request = GenerationRequest::new(MODEL.to_string(),PROMPT.to_string()).images(images);

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        match ollama.generate(request).await {
            Ok(response) => println!("Ollama response: {}", response.response),
            Err(e) => eprintln!("Error from Ollama: {}", e),
        }
    });
    Ok(())
}


fn save_image_buffer(
    image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    output_path: &str
) -> Result<(), image::ImageError> {
    // Save the image buffer to a file
    image_buffer.save(output_path)?;

    println!("Image saved successfully to: {}", output_path);
    Ok(())
}


// Function to download the image
async fn download_image(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = get(url).await?;
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}


async fn generate_stream_and_print_to_stdout(request:GenerationRequest,ollama:&Ollama){
    let mut stream = ollama.generate_stream(request).await.unwrap();
    let mut stdout = stdout();


    while let Some(res) = stream.next().await {
        let responses = res.unwrap();
        for resp in responses {
            stdout.write(resp.response.as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        }
    }
}

// async fn process_batch(ollama: &Ollama, batch: &[ImageBuffer<Rgb<u8>, Vec<u8>>]) -> Result<(), Box<dyn std::error::Error>> {
//     println!("Processing batch of {} frames", batch.len());
//
//     // Here you would typically combine the frames into a single input for Ollama
//     // For this example, we'll just concatenate descriptions of each frame
//     let mut combined_input = String::new();
//     for (i, frame) in batch.iter().enumerate() {
//         combined_input.push_str(&format!("Frame {}: {}x{} image. ", i + 1, frame.width(), frame.height()));
//     }
//
//     // let request =
//     //             GenerationRequest::new(MODEL.to_string(), PROMPT.to_string()).add_image(image);
//     let request = GenerationRequest::new(MODEL.to_string(), combined_input);
//     generate_stream_and_print_to_stdout(request,ollama).await;
//
//     Ok(())
// }


/*

fn main()-> Result<(), Box<dyn std::error::Error>>{

    let rt = Runtime::new().unwrap();
    ffmpeg::init()?;
    println!("Starting");

    let mut ictx = ffmpeg::format::input(&VIDEO_PATH)?;
    let input = ictx.streams().best(ffmpeg::media::Type::Video).ok_or("No video stream found")?;
    let video_stream_index = input.index();

    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;
    let ollama = Ollama::default();

    rt.block_on(async {
        let mut frame_count = 0;
        let mut batch = Vec::new();
        let mut receive_and_process_frames = |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
            let mut decoded = ffmpeg::util::frame::Video::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                frame_count += 1;

                if frame_count % FRAME_INTERVAL == 0 {
                    println!("Processing frame {}", frame_count);

                    // Convert the frame to an RGB image
                    let width = decoded.width();
                    let height = decoded.height();
                    let mut buffer = ImageBuffer::new(width, height);
                    for y in 0..height {
                        for x in 0..width {
                            let pixel = decoded.data(0)[(y * decoded.stride(0) + x * 3) as usize];
                            buffer.put_pixel(x, y, Rgb([pixel, pixel, pixel]));
                        }
                    }

                    batch.push(buffer);

                    // If we've reached the batch size, process the batch
                    if batch.len() >= BATCH_SIZE {
                        process_batch(&ollama, &batch).await?;
                        batch.clear();
                    }
                }
            }
            Ok(())
        };

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                receive_and_process_frames(&mut decoder)?;
            }
        }
        decoder.send_eof()?;
        receive_and_process_frames(&mut decoder)?;

        // Process any remaining frames in the batch
        if !batch.is_empty() {
            process_batch(&ollama, &batch)?;
        }

        println!("Processed {} frames", frame_count);
        Ok(());


    });
    Ok(())
}


*/



// Download the image and encode it to base64
// println!("Downloading Image");
// let bytes = match download_image(IMAGE_URL).await {
//     Ok(b) => b,
//     Err(e) => {
//         eprintln!("Failed to download image: {}", e);
//         return;
//     }
// };
// let base64_image = base64::engine::general_purpose::STANDARD.encode(&bytes);
//
// // Create an Image struct from the base64 string
// let image = Image::from_base64(&base64_image);

// Create a GenerationRequest with the model and prompt, adding the image
// let request =
//     GenerationRequest::new(MODEL.to_string(), PROMPT.to_string()).add_image(image);

// Send the request to the model and get the response



// // Function to send the request to the model
// async fn send_request(
//     request: GenerationRequest,
// ) -> Result<GenerationResponse, Box<dyn std::error::Error>> {
//     let response = ollama.generate(request).await?;
//     Ok(response)
// }
