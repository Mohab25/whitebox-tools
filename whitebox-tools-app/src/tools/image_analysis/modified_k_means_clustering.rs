/*
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: 30/12/2017
Last Modified: 24/02/2019
License: MIT
*/

use whitebox_raster::*;
use whitebox_common::rendering::html::*;
use whitebox_common::rendering::LineGraph;
use crate::tools::*;
use num_cpus;
use rand::prelude::*;
use std::env;
use std::f64;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::io::{Error, ErrorKind};
use std::path;
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

/// This modified k-means algorithm is similar to that described by Mather and Koch (2011).
/// The main difference between the traditional k-means and this technique is that the user
/// does not need to specify the desired number of classes/clusters prior to running the
/// tool. Instead, the algorithm initializes with a very liberal overestimate of the number
/// of classes and then merges classes that have cluster centres that are separated by less
/// than a user-defined threshold. The main difference between this algorithm and the ISODATA
/// technique is that clusters can not be broken apart into two smaller clusters.
///
/// # Reference
/// Mather, P. M., & Koch, M. (2011). Computer processing of remotely-sensed images: an
/// introduction. John Wiley & Sons.
///
/// # See Also
/// `KMeansClustering`
pub struct ModifiedKMeansClustering {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl ModifiedKMeansClustering {
    pub fn new() -> ModifiedKMeansClustering {
        // public constructor
        let name = "ModifiedKMeansClustering".to_string();
        let toolbox = "Machine Learning".to_string();
        let description =
            "Performs a modified k-means clustering operation on a multi-spectral dataset."
                .to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input Files".to_owned(),
            flags: vec!["-i".to_owned(), "--inputs".to_owned()],
            description: "Input raster files.".to_owned(),
            parameter_type: ParameterType::FileList(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output Raster File".to_owned(),
            flags: vec!["-o".to_owned(), "--output".to_owned()],
            description: "Output raster file.".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output HTML Report File".to_owned(),
            flags: vec!["--out_html".to_owned()],
            description: "Output HTML report file.".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Html),
            default_value: None,
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Initial Num. of Clusters".to_owned(),
            flags: vec!["--start_clusters".to_owned()],
            description: "Initial number of clusters".to_owned(),
            parameter_type: ParameterType::Integer,
            default_value: Some("1000".to_owned()),
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Cluster Merger Distance".to_owned(),
            flags: vec!["--merge_dist".to_owned()],
            description: "Cluster merger distance".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: None,
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Max. Iterations".to_owned(),
            flags: vec!["--max_iterations".to_owned()],
            description: "Maximum number of iterations".to_owned(),
            parameter_type: ParameterType::Integer,
            default_value: Some("10".to_owned()),
            optional: true,
        });

        parameters.push(ToolParameter {
            name: "Percent Class Change Threshold".to_owned(),
            flags: vec!["--class_change".to_owned()],
            description: "Minimum percent of cells changed between iterations before completion"
                .to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("2.0".to_owned()),
            optional: true,
        });

        // parameters.push(ToolParameter{
        //     name: "How to Initialize Cluster Centres?".to_owned(),
        //     flags: vec!["--initialize".to_owned()],
        //     description: "How to initialize cluster centres?".to_owned(),
        //     parameter_type: ParameterType::OptionList(vec!["diagonal".to_owned(), "random".to_owned()]),
        //     default_value: Some("diagonal".to_owned()),
        //     optional: true
        // });

        // parameters.push(ToolParameter{
        //     name: "Min. Class Size".to_owned(),
        //     flags: vec!["--min_class_size".to_owned()],
        //     description: "Minimum class size, in pixels".to_owned(),
        //     parameter_type: ParameterType::Integer,
        //     default_value: Some("10".to_owned()),
        //     optional: true
        // });

        let sep: String = path::MAIN_SEPARATOR.to_string();
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut parent = env::current_exe().unwrap();
        parent.pop();
        let p = format!("{}", parent.display());
        let mut short_exe = e
            .replace(&p, "")
            .replace(".exe", "")
            .replace(".", "")
            .replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(">>.*{} -r={} -v --wd='*path*to*data*' -i='image1.tif;image2.tif;image3.tif' -o=output.tif --out_html=report.html --start_clusters=100 --merge_dist=30.0 --max_iterations=25 --class_change=1.5", short_exe, name).replace("*", &sep);

        ModifiedKMeansClustering {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for ModifiedKMeansClustering {
    fn get_source_file(&self) -> String {
        String::from(file!())
    }

    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        match serde_json::to_string(&self.parameters) {
            Ok(json_str) => return format!("{{\"parameters\":{}}}", json_str),
            Err(err) => return format!("{:?}", err),
        }
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn get_toolbox(&self) -> String {
        self.toolbox.clone()
    }

    fn run<'a>(
        &self,
        args: Vec<String>,
        working_directory: &'a str,
        verbose: bool,
    ) -> Result<(), Error> {
        let mut input_files_str = String::new();
        let mut output_file = String::new();
        let mut output_html_file = String::new();
        let mut num_classes = 1000usize;
        let mut max_iterations = 10usize;
        let mut percent_changed_threshold = 5f64;
        // let mut initialization_mode = 1;
        // let mut min_class_size = 10;
        let mut merger_dist = 1f64;

        if args.len() == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Tool run with no parameters.",
            ));
        }
        for i in 0..args.len() {
            let mut arg = args[i].replace("\"", "");
            arg = arg.replace("\'", "");
            let cmd = arg.split("="); // in case an equals sign was used
            let vec = cmd.collect::<Vec<&str>>();
            let mut keyval = false;
            if vec.len() > 1 {
                keyval = true;
            }
            let flag_val = vec[0].to_lowercase().replace("--", "-");
            if flag_val == "-i" || flag_val == "-inputs" || flag_val == "-input" {
                if keyval {
                    input_files_str = vec[1].to_string();
                } else {
                    input_files_str = args[i + 1].to_string();
                }
            } else if flag_val == "-o" || flag_val == "-output" {
                if keyval {
                    output_file = vec[1].to_string();
                } else {
                    output_file = args[i + 1].to_string();
                }
            } else if flag_val == "-out_html" {
                if keyval {
                    output_html_file = vec[1].to_string();
                } else {
                    output_html_file = args[i + 1].to_string();
                }
            } else if flag_val == "-start_clusters" {
                if keyval {
                    num_classes = vec[1]
                        .to_string()
                        .parse::<f32>()
                        .expect(&format!("Error parsing {}", flag_val))
                        as usize;
                } else {
                    num_classes = args[i + 1]
                        .to_string()
                        .parse::<f32>()
                        .expect(&format!("Error parsing {}", flag_val))
                        as usize;
                }
            } else if flag_val == "-merge_dist" || flag_val == "-merger_dist" {
                if keyval {
                    merger_dist = vec[1]
                        .to_string()
                        .parse::<f64>()
                        .expect(&format!("Error parsing {}", flag_val));
                } else {
                    merger_dist = args[i + 1]
                        .to_string()
                        .parse::<f64>()
                        .expect(&format!("Error parsing {}", flag_val));
                }
                merger_dist *= merger_dist;
            } else if flag_val == "-max_iterations" {
                if keyval {
                    max_iterations = vec[1]
                        .to_string()
                        .parse::<f32>()
                        .expect(&format!("Error parsing {}", flag_val))
                        as usize;
                } else {
                    max_iterations = args[i + 1]
                        .to_string()
                        .parse::<f32>()
                        .expect(&format!("Error parsing {}", flag_val))
                        as usize;
                }
            } else if flag_val == "-class_change" {
                if keyval {
                    percent_changed_threshold = vec[1]
                        .to_string()
                        .parse::<f64>()
                        .expect(&format!("Error parsing {}", flag_val));
                } else {
                    percent_changed_threshold = args[i + 1]
                        .to_string()
                        .parse::<f64>()
                        .expect(&format!("Error parsing {}", flag_val));
                }
                // } else if flag_val == "-initialize" {
                //     if keyval {
                //         if vec[1].to_string().to_lowercase().contains("rand") {
                //             initialization_mode = 0;
                //         }
                //     } else {
                //         if args[i + 1].to_string().to_lowercase().contains("diag") {
                //             initialization_mode = 1;
                //         }
                //     }
                // } else if flag_val == "-min_class_size" {
                //     if keyval {
                //         min_class_size = vec[1].to_string().parse::<f32>().expect(&format!("Error parsing {}", flag_val)) as usize;
                //     } else {
                //         min_class_size = args[i + 1].to_string().parse::<f32>().expect(&format!("Error parsing {}", flag_val)) as usize;
                //     }
            }
        }

        if verbose {
            let tool_name = self.get_tool_name();
            let welcome_len = format!("* Welcome to {} *", tool_name).len().max(28); 
            // 28 = length of the 'Powered by' by statement.
            println!("{}", "*".repeat(welcome_len));
            println!("* Welcome to {} {}*", tool_name, " ".repeat(welcome_len - 15 - tool_name.len()));
            println!("* Powered by WhiteboxTools {}*", " ".repeat(welcome_len - 28));
            println!("* www.whiteboxgeo.com {}*", " ".repeat(welcome_len - 23));
            println!("{}", "*".repeat(welcome_len));
        }

        let sep: String = path::MAIN_SEPARATOR.to_string();

        let mut progress: usize;
        let mut old_progress: usize = 1;

        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }

        if !output_html_file.contains(&sep) {
            output_html_file = format!("{}{}", working_directory, output_html_file);
        }

        if !output_html_file.ends_with(".html") && !output_html_file.is_empty() {
            output_html_file.push_str(".html");
        }

        let mut cmd = input_files_str.split(";");
        let mut input_files = cmd.collect::<Vec<&str>>();
        if input_files.len() == 1 {
            cmd = input_files_str.split(",");
            input_files = cmd.collect::<Vec<&str>>();
        }
        let num_files = input_files.len();
        if num_files < 2 {
            return Err(Error::new(ErrorKind::InvalidInput,
                                "There is something incorrect about the input files. At least two inputs are required to operate this tool."));
        }

        if max_iterations < 2 || max_iterations > 250 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Maximum iterations should be between 2 and 250.",
            ));
        }

        if percent_changed_threshold < 0f64 || percent_changed_threshold > 25f64 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "class_change flag should be between 0.0 and 25.0.",
            ));
        }

        if merger_dist < 0f64 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Cluster merger distance must be positive.",
            ));
        }

        let start = Instant::now();

        let mut rows = -1isize;
        let mut columns = -1isize;

        let mut nodata: Vec<f64> = Vec::with_capacity(num_files);
        let mut minimum: Vec<f64> = Vec::with_capacity(num_files);
        let mut maximum: Vec<f64> = Vec::with_capacity(num_files);
        let mut input_raster: Vec<Raster> = Vec::with_capacity(num_files);

        for i in 0..num_files {
            println!("Reading file {} of {}", i + 1, num_files);
            if !input_files[i].trim().is_empty() {
                let mut input_file = input_files[i].trim().to_owned();
                if !input_file.contains(&sep) && !input_file.contains("/") {
                    input_file = format!("{}{}", working_directory, input_file);
                }
                input_raster.push(Raster::new(&input_file, "r")?);
                nodata.push(input_raster[i].configs.nodata);
                minimum.push(input_raster[i].configs.minimum);
                maximum.push(input_raster[i].configs.maximum);

                if rows == -1 || columns == -1 {
                    rows = input_raster[i].configs.rows as isize;
                    columns = input_raster[i].configs.columns as isize;
                    if num_classes < 2 || num_classes as isize > (rows * columns) {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Number of classes should be between 2 and rows x columns.",
                        ));
                    }
                // if min_class_size > ((rows * columns) as usize / num_classes) {
                //     return Err(Error::new(ErrorKind::InvalidInput,
                //         "Min class size should be less than rows x columns / num_classes."));
                // }
                } else {
                    if input_raster[i].configs.rows as isize != rows
                        || input_raster[i].configs.columns as isize != columns
                    {
                        return Err(Error::new(ErrorKind::InvalidInput,
                            "All input images must share the same dimensions (rows and columns) and spatial extent."));
                    }
                }
            }
        }

        if rows == -1 || columns == -1 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Something is incorrect with the specified input files.",
            ));
        }

        let out_nodata = nodata[0];
        let mut output = Raster::initialize_using_file(&output_file, &input_raster[0]);
        let mut class_centres = vec![vec![0f64; num_files]; num_classes];

        // if initialization_mode == 0 {
        // initialize the class centres randomly
        let mut rng = thread_rng();
        for a in 0..num_classes {
            let row = rng.gen_range(0, rows); // Range::new(0, rows).ind_sample(&mut rng);
            let col = rng.gen_range(0, columns); // Range::new(0, columns).ind_sample(&mut rng);
            for i in 0..num_files {
                //let between = Range::new(minimum[i], maximum[i]);
                // class_centres[a][i] = between.ind_sample(&mut rng);
                class_centres[a][i] = input_raster[i].get_value(row, col);
            }
        }
        // } else {
        //     let (mut range, mut spacing): (f64, f64);
        //     for a in 0..num_classes {
        //         for i in 0..num_files {
        //             range = maximum[i] - minimum[i];
        //             spacing = range / num_classes as f64;
        //             class_centres[a][i] = minimum[i] + spacing * a as f64;
        //         }
        //     }
        // }

        let input_raster = Arc::new(input_raster);
        let mut which_class = 0usize;
        let mut percent_changed: f64;
        let mut class_n = vec![0usize; num_classes];
        let mut z: f64;
        let mut class: usize;
        let mut n_counted = false;
        let mut n = 0f64;
        let nodata = Arc::new(nodata);
        let mut xdata = vec![vec![0f64; max_iterations]; 1];
        let mut ydata = vec![vec![0f64; max_iterations]; 1];
        for loop_num in 0..max_iterations {
            xdata[0][loop_num] = (loop_num + 1) as f64;

            // assign each pixel to a class
            let mut class_centre_data = vec![vec![0f64; num_files]; num_classes];
            class_n = vec![0usize; num_classes];

            let mut cells_changed = 0f64;

            let mut num_procs = num_cpus::get() as isize;
            let configs = whitebox_common::configs::get_configs()?;
            let max_procs = configs.max_procs;
            if max_procs > 0 && max_procs < num_procs {
                num_procs = max_procs;
            }
            let centres = Arc::new(class_centres.clone());
            let (tx, rx) = mpsc::channel();
            for tid in 0..num_procs {
                let input_raster = input_raster.clone();
                let centres = centres.clone();
                let nodata = nodata.clone();
                let tx = tx.clone();
                thread::spawn(move || {
                    for row in (0..rows).filter(|r| r % num_procs == tid) {
                        let mut data = vec![-1isize; columns as usize];
                        let mut is_valid_data: bool;
                        let mut min_dist: f64;
                        let mut dist: f64;
                        let mut value = vec![0f64; num_files];
                        let mut class_centre_data = vec![vec![0f64; num_files]; num_classes];
                        for col in 0..columns {
                            is_valid_data = true;
                            for i in 0..num_files {
                                value[i] = input_raster[i].get_value(row, col);
                                if value[i] == nodata[i] {
                                    is_valid_data = false;
                                    break;
                                }
                            }
                            if is_valid_data {
                                // calculate the squared distance to each of the centroids
                                // and assign the pixel the value of the nearest centroid.
                                min_dist = f64::INFINITY;
                                for a in 0..num_classes {
                                    dist = 0f64;
                                    for i in 0..num_files {
                                        dist +=
                                            (value[i] - centres[a][i]) * (value[i] - centres[a][i]);
                                    }
                                    if dist < min_dist {
                                        min_dist = dist;
                                        which_class = a;
                                    }
                                }
                                data[col as usize] = which_class as isize;

                                for i in 0..num_files {
                                    class_centre_data[which_class][i] += value[i];
                                }
                            }
                        }
                        tx.send((row, data, class_centre_data)).unwrap();
                    }
                });
            }

            for r in 0..rows {
                let (row, data, ccd) = rx.recv().expect("Error receiving data from thread.");
                for col in 0..columns {
                    if data[col as usize] >= 0 {
                        if !n_counted {
                            n += 1f64;
                        }
                        which_class = data[col as usize] as usize;
                        z = output.get_value(row, col);
                        class = z as usize - 1usize;
                        if z == out_nodata || which_class != class {
                            cells_changed += 1f64;
                            output.set_value(row, col, which_class as f64 + 1f64);
                        }

                        class_n[which_class] += 1;
                    }
                }

                for a in 0..num_classes {
                    for i in 0..num_files {
                        class_centre_data[a][i] += ccd[a][i];
                    }
                }

                if verbose {
                    progress = (100.0_f64 * r as f64 / (rows - 1) as f64) as usize;
                    if progress != old_progress {
                        println!(
                            "Progress (loop {} of {}): {}%",
                            loop_num + 1,
                            max_iterations,
                            progress
                        );
                        old_progress = progress;
                    }
                }
            }

            n_counted = true;

            // Update the class centroids
            for a in 0..num_classes {
                if class_n[a] > 0 {
                    for i in 0..num_files {
                        class_centres[a][i] = class_centre_data[a][i] / class_n[a] as f64;
                    }
                }
            }

            // See if any of the class centroids are close enough to be merged.
            let mut weight1: f64;
            let mut weight2: f64;
            let mut dist: f64;
            for a in 0..num_classes {
                if class_n[a] > 0 {
                    for b in a + 1..num_classes {
                        if class_n[b] > 0 {
                            dist = 0f64;
                            for i in 0..num_files {
                                dist += (class_centres[a][i] - class_centres[b][i])
                                    * (class_centres[a][i] - class_centres[b][i]);
                            }
                            if dist < merger_dist {
                                // calculate the weights
                                weight1 = class_n[a] as f64 / (class_n[a] + class_n[b]) as f64;
                                weight2 = class_n[b] as f64 / (class_n[a] + class_n[b]) as f64;
                                // adjust the centroid position
                                for i in 0..num_files {
                                    class_centres[a][i] = class_centres[a][i] * weight1
                                        + class_centres[b][i] * weight2;
                                    class_centres[b][i] = 0f64;
                                }
                                // update the class size
                                class_n[a] += class_n[b];
                                class_n[b] = 0;
                            }
                        }
                    }
                }
            }

            // remove the empty clusters
            for a in (0..num_classes).rev() {
                if class_n[a] == 0 {
                    class_centres.remove(a);
                    class_n.remove(a);
                }
            }

            num_classes = class_n.len();

            println!("Number of clusters: {}", num_classes);

            percent_changed = 100f64 * cells_changed / n;
            ydata[0][loop_num] = percent_changed;
            println!(
                "Cells changed {} ({:.4} percent)",
                cells_changed, percent_changed
            );
            if percent_changed < percent_changed_threshold {
                break;
            }
        }

        let elapsed_time = get_formatted_elapsed_time(start);
        output.configs.data_type = DataType::I16;
        output.configs.palette = "qual.plt".to_string();
        output.configs.photometric_interp = PhotometricInterpretation::Categorical;
        output.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output.add_metadata_entry(format!("Num. clusters: {}", num_classes));
        output.add_metadata_entry(format!("Num. bands: {}", num_files));
        output.add_metadata_entry(format!("max_iterations: {}", max_iterations));
        output.add_metadata_entry(format!("class_change: {}", percent_changed_threshold));
        output.add_metadata_entry(format!("merger_dist: {}", merger_dist.sqrt()));
        output.add_metadata_entry(format!("Elapsed Time (including I/O): {}", elapsed_time));

        if verbose {
            println!("Saving data...")
        };
        let _ = match output.write() {
            Ok(_) => {
                if verbose {
                    println!("Output file written")
                }
            }
            Err(e) => return Err(e),
        };
        if verbose {
            println!(
                "{}",
                &format!("Elapsed Time (including I/O): {}", elapsed_time)
            );
        }

        if !output_html_file.trim().is_empty() {
            let f = File::create(output_html_file.clone())?;
            let mut writer = BufWriter::new(f);

            writer.write_all(&r#"<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\">
            <html>
                <head>
                    <meta content=\"text/html; charset=UTF-8\" http-equiv=\"content-type\">
                    <title>Modified k-Means Clustering</title>"#.as_bytes())?;

            // get the style sheet
            writer.write_all(&get_css().as_bytes())?;

            writer.write_all(
                &r#"
                </head>
                <body>
                    <h1>Modified k-Means Clustering Report</h1>
                    <p>"#
                    .as_bytes(),
            )?;

            writer
                .write_all(&format!("<strong>Num. bands</strong>: {}<br>", num_files).as_bytes())?;
            for i in 0..num_files {
                writer.write_all(
                    &format!(
                        "<strong>Image {}</strong>: {}<br>",
                        i + 1,
                        input_files[i].clone()
                    )
                    .as_bytes(),
                )?;
            }
            writer.write_all(
                &format!("<strong>Num. clusters</strong>: {}<br>", num_classes).as_bytes(),
            )?;
            writer.write_all(
                &format!("<strong>Max. iterations</strong>: {}<br>", max_iterations).as_bytes(),
            )?;
            writer.write_all(
                &format!(
                    "<strong>Percent change threshold</strong>: {:.3}%<br>",
                    percent_changed_threshold
                )
                .as_bytes(),
            )?;
            writer.write_all(
                &format!(
                    "<strong>Cluster merger distance</strong>: {}<br>",
                    merger_dist.sqrt()
                )
                .as_bytes(),
            )?;
            // if initialization_mode == 0 {
            //     writer.write_all("<strong>Initialize method</strong>: random<br>".to_string().as_bytes())?;
            // } else {
            //     writer.write_all("<strong>Initialize method</strong>: diagonal<br>".to_string().as_bytes())?;
            // }

            writer.write_all("</p>".as_bytes())?;

            ////////////////////////
            // Cluster Size table //
            ////////////////////////
            writer.write_all("<p><table>".as_bytes())?;
            writer.write_all("<caption>Cluster Size</caption>".as_bytes())?;
            writer.write_all("<tr><th>Cluster</th><th>Num. Pixels</th></tr>".as_bytes())?;
            for a in 0..num_classes {
                writer.write_all(
                    &format!(
                        "<tr><td>{}</td><td class=\"numberCell\">{}</td></tr>",
                        a + 1,
                        class_n[a]
                    )
                    .as_bytes(),
                )?;
            }
            writer.write_all("</table></p>".as_bytes())?;

            /////////////////////////////
            // Cluster Centroid Vector //
            /////////////////////////////
            writer.write_all("<p><table>".as_bytes())?;
            writer.write_all("<caption>Cluster Centroid Vector</caption>".as_bytes())?;

            let mut s = String::from("<tr><th>Cluster</th>");
            for i in 0..num_files {
                s.push_str(&format!("<th>Image {}</th>", i + 1));
            }
            s.push_str("</tr>");
            writer.write_all(s.as_bytes())?;

            for a in 0..num_classes {
                let mut s = format!("<tr><td>{}</td>", a + 1);
                for i in 0..num_files {
                    s.push_str(&format!(
                        "<td class=\"numberCell\">{:.3}</td>",
                        class_centres[a][i]
                    ));
                }
                s.push_str("</tr>");
                writer.write_all(s.as_bytes())?;
            }
            writer.write_all("</table></p>".as_bytes())?;

            ////////////////////////////////////////
            // Cluster Centroid Distance Analysis //
            ////////////////////////////////////////
            writer.write_all("<p><table>".as_bytes())?;
            writer.write_all("<caption>Cluster Centroid Distance Analysis</caption>".as_bytes())?;
            let mut s = String::from("<tr><th></th>");
            for a in 0..num_classes {
                s.push_str(&format!("<th>Cluster {}</th>", a + 1));
            }
            s.push_str("</tr>");
            writer.write_all(s.as_bytes())?;

            for a in 0..num_classes {
                let mut s = format!("<tr><td class=\"header\">Cluster {}</td>", a + 1);
                for b in 0..num_classes {
                    if b >= a {
                        let mut dist = 0f64;
                        for i in 0..num_files {
                            dist += (class_centres[a][i] - class_centres[b][i])
                                * (class_centres[a][i] - class_centres[b][i]);
                        }
                        s.push_str(&format!("<td class=\"numberCell\">{:.3}</td>", dist.sqrt()));
                    } else {
                        s.push_str("<td></td>");
                    }
                }
                s.push_str("</tr>");
                writer.write_all(s.as_bytes())?;
            }
            writer.write_all("</table></p>".as_bytes())?;

            //////////////////////
            // convergence plot //
            //////////////////////
            for loop_num in (0..max_iterations).rev() {
                if xdata[0][loop_num] == 0f64 {
                    xdata[0].remove(loop_num);
                    ydata[0].remove(loop_num);
                }
            }
            writer.write_all("<br><br><h2>Convergence Plot</h2>".as_bytes())?;
            let graph = LineGraph {
                parent_id: "graph".to_string(),
                width: 500f64,
                height: 450f64,
                data_x: xdata.clone(),
                data_y: ydata.clone(),
                series_labels: vec!["Line 1".to_string()].clone(),
                x_axis_label: "Iteration".to_string(),
                y_axis_label: "Cells with class values changed (%)".to_string(),
                draw_points: true,
                draw_gridlines: true,
                draw_legend: false,
                draw_grey_background: false,
            };

            writer.write_all(
                &format!("<div id='graph' align=\"center\">{}</div>", graph.get_svg()).as_bytes(),
            )?;

            writer.write_all("</body>".as_bytes())?;
            writer.write_all("</html>".as_bytes())?;

            let _ = writer.flush();

            if verbose {
                if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
                    let output = Command::new("open")
                        .arg(output_html_file.clone())
                        .output()
                        .expect("failed to execute process");

                    let _ = output.stdout;
                } else if cfg!(target_os = "windows") {
                    // let output = Command::new("cmd /c start")
                    let output = Command::new("explorer.exe")
                        .arg(output_html_file.clone())
                        .output()
                        .expect("failed to execute process");

                    let _ = output.stdout;
                } else if cfg!(target_os = "linux") {
                    let output = Command::new("xdg-open")
                        .arg(output_html_file.clone())
                        .output()
                        .expect("failed to execute process");

                    let _ = output.stdout;
                }

                println!("Complete! Please see {} for output.", output_html_file);
            }
        }

        Ok(())
    }
}
