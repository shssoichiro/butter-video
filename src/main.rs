#![warn(clippy::all)]

use std::{env, fs, mem::size_of, path::Path, process::Command};

use av_metrics_decoders::{
    CastFromPrimitive,
    ChromaSampling,
    Decoder,
    FrameInfo,
    Pixel,
    Y4MDecoder,
};
use average::{Estimate, Quantile};
use clap::{App, Arg, ArgMatches, SubCommand};
use image::{ImageBuffer, RgbImage};
use tempfile::Builder;
use yuv::{
    color::{MatrixCoefficients, Range},
    convert::RGBConvert,
    YUV,
};

fn main() {
    let args = App::new("butter-video")
        .about("Calculates butteraugli and ssimulacra metrics for videos")
        .subcommand(
            SubCommand::with_name("butter")
                .about("Calculate butteraugli")
                .arg(Arg::with_name("input1").required(true).index(1))
                .arg(Arg::with_name("input2").required(true).index(2)),
        )
        .subcommand(
            SubCommand::with_name("ssimulacra")
                .about("Calculate ssimulacra")
                .arg(Arg::with_name("input1").required(true).index(1))
                .arg(Arg::with_name("input2").required(true).index(2)),
        )
        .get_matches();

    match args.subcommand_name().unwrap() {
        "butter" => compute_butter(args.subcommand_matches("butter").unwrap()),
        "ssimulacra" => compute_ssimulacra(args.subcommand_matches("ssimulacra").unwrap()),
        _ => unreachable!(),
    };
}

fn compute_butter(args: &ArgMatches) {
    let butteraugli_path =
        env::var("BUTTERAUGLI_PATH").unwrap_or_else(|_| "butteraugli".to_string());
    let input1 = Path::new(args.value_of("input1").unwrap());
    let input2 = Path::new(args.value_of("input2").unwrap());
    run_metric(&butteraugli_path, input1, input2);
}

fn compute_ssimulacra(args: &ArgMatches) {
    let ssimulacra_path = env::var("SSIMULACRA_PATH").unwrap_or_else(|_| "ssimulacra".to_string());
    let input1 = Path::new(args.value_of("input1").unwrap());
    let input2 = Path::new(args.value_of("input2").unwrap());
    run_metric(&ssimulacra_path, input1, input2);
}

fn run_metric(base_command: &str, input1: &Path, input2: &Path) {
    let mut dec1 = Y4MDecoder::new(input1).expect("Failed to open file");
    let details1 = dec1.get_video_details();
    let mut dec2 = Y4MDecoder::new(input2).expect("Failed to open file");
    let details2 = dec2.get_video_details();
    assert_eq!(details1.height, details2.height);
    assert_eq!(details1.width, details2.width);

    let mut sum = 0.0f64;
    let mut norms = vec![];
    let mut frameno = 0;

    loop {
        match (details1.bit_depth, details2.bit_depth) {
            (8, 8) => {
                let frame1 = dec1.read_video_frame::<u8>();
                let frame2 = dec2.read_video_frame::<u8>();
                if frame1.is_none() || frame2.is_none() {
                    if frame1.is_some() || frame2.is_some() {
                        eprintln!(
                            "WARNING: Clips did not match in length! Ending at frame {}",
                            frameno
                        );
                    }
                    break;
                }
                let (score, norm) = compare_frame(base_command, &frame1.unwrap(), &frame2.unwrap());
                sum += score;
                if let Some(norm) = norm {
                    norms.push(norm);
                }
            }
            (8, _) => {
                let frame1 = dec1.read_video_frame::<u8>();
                let frame2 = dec2.read_video_frame::<u16>();
                if frame1.is_none() || frame2.is_none() {
                    if frame1.is_some() || frame2.is_some() {
                        eprintln!(
                            "WARNING: Clips did not match in length! Ending at frame {}",
                            frameno
                        );
                    }
                    break;
                }
                let (score, norm) = compare_frame(base_command, &frame1.unwrap(), &frame2.unwrap());
                sum += score;
                if let Some(norm) = norm {
                    norms.push(norm);
                }
            }
            (_, 8) => {
                let frame1 = dec1.read_video_frame::<u16>();
                let frame2 = dec2.read_video_frame::<u8>();
                if frame1.is_none() || frame2.is_none() {
                    if frame1.is_some() || frame2.is_some() {
                        eprintln!(
                            "WARNING: Clips did not match in length! Ending at frame {}",
                            frameno
                        );
                    }
                    break;
                }
                let (score, norm) = compare_frame(base_command, &frame1.unwrap(), &frame2.unwrap());
                sum += score;
                if let Some(norm) = norm {
                    norms.push(norm);
                }
            }
            (_, _) => {
                let frame1 = dec1.read_video_frame::<u16>();
                let frame2 = dec2.read_video_frame::<u16>();
                if frame1.is_none() || frame2.is_none() {
                    if frame1.is_some() || frame2.is_some() {
                        eprintln!(
                            "WARNING: Clips did not match in length! Ending at frame {}",
                            frameno
                        );
                    }
                    break;
                }
                let (score, norm) = compare_frame(base_command, &frame1.unwrap(), &frame2.unwrap());
                sum += score;
                if let Some(norm) = norm {
                    norms.push(norm);
                }
            }
        };

        frameno += 1;
    }

    if frameno == 0 {
        panic!("No frames read");
    }

    let avg_score = sum / frameno as f64;
    println!("Score: {}", avg_score);
    if !norms.is_empty() {
        let mut quant = Quantile::new(0.75);
        for norm in norms {
            quant.add(norm);
        }
        println!("3-norm (75th percentile): {}", quant.quantile());
    }
}

fn compare_frame<T: Pixel, U: Pixel>(
    base_command: &str,
    frame1: &FrameInfo<T>,
    frame2: &FrameInfo<U>,
) -> (f64, Option<f64>) {
    let (_, path1) = Builder::new()
        .suffix(".png")
        .tempfile()
        .unwrap()
        .keep()
        .unwrap();
    let (_, path2) = Builder::new()
        .suffix(".png")
        .tempfile()
        .unwrap()
        .keep()
        .unwrap();
    {
        let image1: RgbImage = ImageBuffer::from_raw(
            frame1.planes[0].cfg.width as u32,
            frame1.planes[0].cfg.height as u32,
            yuv_to_rgb_u8(frame1),
        )
        .unwrap();
        image1.save(&path1).unwrap();

        let image2: RgbImage = ImageBuffer::from_raw(
            frame2.planes[0].cfg.width as u32,
            frame2.planes[0].cfg.height as u32,
            yuv_to_rgb_u8(frame2),
        )
        .unwrap();
        image2.save(&path2).unwrap();
    }
    let output = Command::new(base_command)
        .arg(&path1)
        .arg(&path2)
        .output()
        .unwrap();

    let _ = fs::remove_file(path1);
    let _ = fs::remove_file(path2);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let score = stdout
        .lines()
        .find(|line| !line.is_empty())
        .unwrap()
        .trim()
        .parse::<f64>()
        .unwrap();
    let norm = stdout
        .lines()
        .find(|line| line.starts_with("3-norm"))
        .map(|line| line.split_once(": ").unwrap())
        .map(|(_, val)| val.parse::<f64>().unwrap());
    (score, norm)
}

// butteraugli_main makes us use .ppm files which don't support 8-bit input.
// I hate it, but it means we have to downconvert.
// The results should not be considerably skewed.
fn yuv_to_rgb_u8<T: Pixel>(frame: &FrameInfo<T>) -> Vec<u8> {
    let plane_y = &frame.planes[0];
    let plane_u = &frame.planes[1];
    let plane_v = &frame.planes[2];

    // TODO: Support HDR content
    let colorspace = if plane_y.cfg.height > 576 {
        MatrixCoefficients::BT709
    } else {
        MatrixCoefficients::BT601
    };
    let (ss_x, ss_y) = match frame.chroma_sampling {
        ChromaSampling::Cs400 => {
            return (0..plane_y.cfg.height)
                .flat_map(|y| {
                    (0..plane_y.cfg.width).flat_map(move |x| {
                        let val = if size_of::<T>() == 1 {
                            u8::cast_from(plane_y.p(x, y))
                        } else {
                            (u16::cast_from(plane_y.p(x, y)) >> (frame.bit_depth - 8)) as u8
                        };
                        [val, val, val].into_iter()
                    })
                })
                .collect();
        }
        ChromaSampling::Cs420 => (1, 1),
        ChromaSampling::Cs422 => (0, 1),
        ChromaSampling::Cs444 => (0, 0),
    };

    debug_assert_eq!(frame.bit_depth, 8);
    let converter = RGBConvert::<u8>::new(Range::Limited, colorspace).unwrap();
    (0..plane_y.cfg.height)
        .flat_map(|y| {
            let converter = converter.clone();
            (0..plane_y.cfg.width).flat_map(move |x| {
                let (chroma_x, chroma_y) = (x >> ss_x, y >> ss_y);
                let (y, u, v) = if size_of::<T>() == 1 {
                    (
                        u8::cast_from(plane_y.p(x, y)),
                        u8::cast_from(plane_u.p(chroma_x, chroma_y)),
                        u8::cast_from(plane_v.p(chroma_x, chroma_y)),
                    )
                } else {
                    (
                        (u16::cast_from(plane_y.p(x, y)) >> (frame.bit_depth - 8)) as u8,
                        (u16::cast_from(plane_u.p(chroma_x, chroma_y)) >> (frame.bit_depth - 8))
                            as u8,
                        (u16::cast_from(plane_v.p(chroma_x, chroma_y)) >> (frame.bit_depth - 8))
                            as u8,
                    )
                };
                let yuv = YUV { y, u, v };
                let rgb = converter.to_rgb(yuv);
                [rgb.r, rgb.g, rgb.b].into_iter()
            })
        })
        .collect()
}
