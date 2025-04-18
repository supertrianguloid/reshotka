use crate::bootstrap::get_samples;
use crate::io::{
    load_channel_from_file_folded, load_global_t_from_file, load_wf_observables_from_file,
};
use crate::observables::Measurement;
use crate::spectroscopy::effective_mass;
use crate::statistics::{bin, mean, standard_deviation, standard_error, weighted_mean};
use crate::wilsonflow::{calculate_w, calculate_w0, WilsonFlowObservables};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::generate;
use clap_complete_nushell::Nushell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::stdout};
#[derive(Parser, Debug)]
#[clap(
    name = "Reshotka",
    version = "0.0.1",
    author = "Laurence Sebastian Bowes",
    about = "A tool for SU(2) analysis"
)]
pub struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Calculate the effective mass in a given channel
    ComputeEffectiveMass {
        #[clap(flatten)]
        args: ComputeEffectiveMassArgs,
    },
    /// Given a CSV generated from `compute-effective-mass`, fit a constant to it
    FitEffectiveMass {
        #[clap(flatten)]
        args: FitEffectiveMassArgs,
    },
    BootstrapFitsWithWF {
        #[clap(flatten)]
        args: BootstrapFitsWithWFArgs,
    },
    BootstrapFits {
        #[clap(flatten)]
        args: BootstrapFitsArgs,
    },
    BootstrapFitsRatio {
        #[clap(flatten)]
        args: BootstrapFitsRatioArgs,
    },
    CalculateW0 {
        #[clap(flatten)]
        args: CalculateW0Args,
    },
    Histogram {
        #[clap(flatten)]
        args: HistogramArgs,
    },
    BootstrapError {
        #[clap(flatten)]
        args: BootstrapErrorArgs,
    },
    GenerateCompletions {},
}

#[derive(Parser, Debug)]
struct HMCArgs {
    filename: String,
    #[arg(short, long, value_name = "THERMALISATION", default_value_t = 0)]
    thermalisation: usize,
}

#[derive(Parser, Debug)]
struct WFArgs {
    #[arg(long, value_name = "WILSON_FLOW_FILE")]
    wf_filename: String,
    #[arg(long, value_name = "W_THERMALISATION", default_value_t = 0)]
    wf_thermalisation: usize,
    #[arg(long, value_name = "W_REFERENCE", default_value_t = 1.0)]
    w_ref: f64,
}

#[derive(Parser, Debug)]
struct BinBootstrapArgs {
    #[arg(short, long, value_name = "BOOTSTRAP_SAMPLES", default_value_t = 1000)]
    n_boot: u32,
    #[arg(short, long, value_name = "BIN_WIDTH", default_value_t = 1)]
    binwidth: usize,
}

#[derive(Parser, Debug)]
struct ComputeEffectiveMassArgs {
    #[clap(flatten)]
    hmc: HMCArgs,
    #[clap(flatten)]
    boot: BinBootstrapArgs,
    #[arg(short, long, value_name = "CHANNEL")]
    channel: String,
    #[arg(short, long, value_name = "SOLVER_PRECISION", default_value_t = 1e-15)]
    solver_precision: f64,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MAX")]
    effective_mass_t_max: usize,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MIN")]
    effective_mass_t_min: usize,
}

#[derive(Parser, Debug)]
struct FitEffectiveMassArgs {
    csv_filename: String,
    t1: usize,
    t2: usize,
}

#[derive(Parser, Debug)]
struct HistogramArgs {
    csv_filename: String,
    nbins: usize,
}
#[derive(Parser, Debug)]
struct BootstrapFitsWithWFArgs {
    #[clap(flatten)]
    hmc: HMCArgs,
    #[clap(flatten)]
    wf: WFArgs,
    #[clap(flatten)]
    boot: BinBootstrapArgs,
    #[arg(short, long, value_name = "CHANNEL")]
    channel: String,
    #[arg(short, long, value_name = "SOLVER_PRECISION", default_value_t = 1e-15)]
    solver_precision: f64,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MAX")]
    effective_mass_t_max: usize,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MIN")]
    effective_mass_t_min: usize,
}
#[derive(Parser, Debug)]
struct CalculateW0Args {
    #[clap(flatten)]
    boot: BinBootstrapArgs,
    #[clap(flatten)]
    wf: WFArgs,
}

#[derive(Parser, Debug)]
struct BootstrapFitsArgs {
    #[clap(flatten)]
    hmc: HMCArgs,
    #[clap(flatten)]
    boot: BinBootstrapArgs,
    #[arg(short, long, value_name = "CHANNEL")]
    channel: String,
    #[arg(short, long, value_name = "SOLVER_PRECISION", default_value_t = 1e-15)]
    solver_precision: f64,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MAX")]
    effective_mass_t_max: usize,
    #[arg(long, value_name = "EFFECTIVE_MASS_T_MIN")]
    effective_mass_t_min: usize,
}

#[derive(Parser, Debug)]
struct BootstrapFitsRatioArgs {
    #[clap(flatten)]
    hmc: HMCArgs,
    #[clap(flatten)]
    boot: BinBootstrapArgs,
    #[arg(long, value_name = "NUMERATOR_CHANNEL")]
    numerator_channel: String,
    #[arg(long, value_name = "DENOMINATOR_CHANNEL")]
    denominator_channel: String,
    #[arg(short, long, value_name = "SOLVER_PRECISION", default_value_t = 1e-15)]
    solver_precision: f64,
    #[arg(long, value_name = "NUMERATOR_EFFECTIVE_MASS_T_MAX")]
    numerator_effective_mass_t_max: usize,
    #[arg(long, value_name = "NUMERATOR_EFFECTIVE_MASS_T_MIN")]
    numerator_effective_mass_t_min: usize,
    #[arg(long, value_name = "DENOMINATOR_EFFECTIVE_MASS_T_MAX")]
    denominator_effective_mass_t_max: usize,
    #[arg(long, value_name = "DENOMINATOR_EFFECTIVE_MASS_T_MIN")]
    denominator_effective_mass_t_min: usize,
}

#[derive(Parser, Debug)]
struct BootstrapErrorArgs {
    csv_filename: String,
    #[arg(short, long, value_name = "BOOTSTRAP_SAMPLES", default_value_t = 1000)]
    n_boot: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct EffectiveMassRow {
    #[serde(rename = "Tau")]
    tau: usize,
    #[serde(rename = "Effective Mass")]
    mass: f64,
    #[serde(rename = "Error")]
    error: f64,
    #[serde(rename = "Failed Samples (%)")]
    failures: f64,
}
#[derive(Debug, Serialize)]
struct EffectiveMassFit {
    #[serde(rename = "Effective Mass Fit")]
    mass: f64,
    #[serde(rename = "Error")]
    error: f64,
}
#[derive(Debug, Serialize, Deserialize)]
struct BootstrapSample {
    #[serde(rename = "Sample")]
    sample: f64,
}

fn fit_effective_mass_command(args: FitEffectiveMassArgs) {
    let mut tau = vec![];
    let mut mass = vec![];
    let mut error = vec![];
    let mut rdr = csv::Reader::from_reader(File::open(args.csv_filename).unwrap());
    for result in rdr.deserialize() {
        let record: EffectiveMassRow = result.unwrap();
        tau.push(record.tau);
        mass.push(record.mass);
        error.push(record.error);
    }
    let offset = tau.iter().position(|&x| x == args.t1).unwrap();
    let index = offset..(offset + args.t2 - args.t1 + 1);
    let fit = weighted_mean(&mass[index.clone()], &error[index]);
    let mut wtr = csv::Writer::from_writer(stdout());
    wtr.serialize(EffectiveMassFit {
        mass: fit.0,
        error: fit.1,
    })
    .unwrap();
    wtr.flush().unwrap();
}

fn compute_effective_mass_command(args: ComputeEffectiveMassArgs) {
    let channel = load_channel_from_file_folded(&args.hmc.filename, &args.channel)
        .thermalise(args.hmc.thermalisation);
    let global_t = load_global_t_from_file(&args.hmc.filename);

    let mut solve_failures = vec![];
    let mut effmass_mean = vec![];
    let mut effmass_error = vec![];
    assert_eq!(global_t, (channel.each_len - 1) * 2);
    for tau in 1..=args.effective_mass_t_max {
        let results: Vec<Result<f64, roots::SearchError>> = (0..args.boot.n_boot)
            .into_par_iter()
            .map(|_| {
                let Measurement {
                    values: mu,
                    errors: _,
                } = channel.get_subsample_mean_stderr(args.boot.binwidth);
                effective_mass(&mu, global_t, tau, args.solver_precision)
            })
            .collect();
        let mut effmass_inner = Vec::with_capacity(args.boot.n_boot as usize);
        let mut nfailures = 0;
        for result in results {
            match result {
                Ok(val) => effmass_inner.push(val),
                Err(_) => nfailures += 1,
            }
        }
        solve_failures.push(nfailures);
        effmass_mean.push(mean(&effmass_inner));
        effmass_error.push(standard_deviation(&effmass_inner, true));
    }
    let mut wtr = csv::Writer::from_writer(stdout());
    for tau in args.effective_mass_t_min..=args.effective_mass_t_max {
        wtr.serialize(EffectiveMassRow {
            tau,
            mass: effmass_mean[tau - 1],
            error: effmass_error[tau - 1],
            failures: solve_failures[tau - 1] as f64 * 100.0 / args.boot.n_boot as f64,
        })
        .unwrap();
        wtr.flush().unwrap();
    }
}

fn bootstrap_fits_with_wf_command(args: BootstrapFitsWithWFArgs) {
    let channel = load_channel_from_file_folded(&args.hmc.filename, &args.channel)
        .thermalise(args.hmc.thermalisation);
    let wf =
        load_wf_observables_from_file(&args.wf.wf_filename).thermalise(args.wf.wf_thermalisation);
    assert_eq!(channel.nconfs, wf.tc.nconfs);
    let global_t = load_global_t_from_file(&args.hmc.filename);
    let mut results_g = vec![];
    let results = (0..args.boot.n_boot)
        .into_par_iter()
        .map(|_| {
            let samples = get_samples(channel.nconfs, args.boot.binwidth);
            let w0 = calculate_w0(
                calculate_w(
                    &wf.get_subsample_mean_stderr_from_samples(
                        samples.clone(),
                        WilsonFlowObservables::T2Esym,
                    )
                    .values,
                    &wf.t,
                ),
                args.wf.w_ref,
            );
            let mut masses = vec![];
            let mu = channel
                .get_subsample_mean_stderr_from_samples(samples)
                .values;
            for tau in args.effective_mass_t_min..(args.effective_mass_t_max + 1) {
                let mass = effective_mass(&mu, global_t, tau, args.solver_precision);
                match mass {
                    Err(_) => return None,
                    Ok(val) => masses.push(val),
                };
            }
            Some(mean(&masses) * w0)
        })
        .collect::<Vec<Option<f64>>>();
    for result in results {
        match result {
            None => {}
            Some(val) => results_g.push(val),
        };
    }
    let mut wtr = csv::Writer::from_writer(stdout());
    for sample in results_g {
        wtr.serialize(BootstrapSample { sample }).unwrap();
    }
    wtr.flush().unwrap();
}
fn bootstrap_fits_command(args: BootstrapFitsArgs) {
    let channel = load_channel_from_file_folded(&args.hmc.filename, &args.channel)
        .thermalise(args.hmc.thermalisation);
    let global_t = load_global_t_from_file(&args.hmc.filename);
    let mut results_g = vec![];
    let results = (0..args.boot.n_boot)
        .into_par_iter()
        .map(|_| {
            let samples = get_samples(channel.nconfs, args.boot.binwidth);
            let mut masses = vec![];
            let mu = channel
                .get_subsample_mean_stderr_from_samples(samples)
                .values;
            for tau in args.effective_mass_t_min..(args.effective_mass_t_max + 1) {
                let mass = effective_mass(&mu, global_t, tau, args.solver_precision);
                match mass {
                    Err(_) => return None,
                    Ok(val) => masses.push(val),
                };
            }
            Some(mean(&masses))
        })
        .collect::<Vec<Option<f64>>>();
    for result in results {
        match result {
            None => {}
            Some(val) => results_g.push(val),
        };
    }
    let mut wtr = csv::Writer::from_writer(stdout());
    for sample in results_g {
        wtr.serialize(BootstrapSample { sample }).unwrap();
    }
    wtr.flush().unwrap();
}
fn bootstrap_fits_ratio_command(args: BootstrapFitsRatioArgs) {
    let numerator_channel =
        load_channel_from_file_folded(&args.hmc.filename, &args.numerator_channel)
            .thermalise(args.hmc.thermalisation);
    let denominator_channel =
        load_channel_from_file_folded(&args.hmc.filename, &args.denominator_channel)
            .thermalise(args.hmc.thermalisation);
    let global_t = load_global_t_from_file(&args.hmc.filename);
    let mut results_g = vec![];
    let results = (0..args.boot.n_boot)
        .into_par_iter()
        .map(|_| {
            let samples = get_samples(numerator_channel.nconfs, args.boot.binwidth);

            let mut num_masses = vec![];
            let num_mu = numerator_channel
                .get_subsample_mean_stderr_from_samples(samples.clone())
                .values;
            for tau in
                args.numerator_effective_mass_t_min..(args.numerator_effective_mass_t_max + 1)
            {
                let mass = effective_mass(&num_mu, global_t, tau, args.solver_precision);
                match mass {
                    Err(_) => return None,
                    Ok(val) => num_masses.push(val),
                };
            }

            let mut denom_masses = vec![];
            let denom_mu = denominator_channel
                .get_subsample_mean_stderr_from_samples(samples)
                .values;
            for tau in
                args.denominator_effective_mass_t_min..(args.denominator_effective_mass_t_max + 1)
            {
                let mass = effective_mass(&denom_mu, global_t, tau, args.solver_precision);
                match mass {
                    Err(_) => return None,
                    Ok(val) => denom_masses.push(val),
                };
            }
            Some(mean(&num_masses) / mean(&denom_masses))
        })
        .collect::<Vec<Option<f64>>>();
    for result in results {
        match result {
            None => {}
            Some(val) => results_g.push(val),
        };
    }
    let mut wtr = csv::Writer::from_writer(stdout());
    for sample in results_g {
        wtr.serialize(BootstrapSample { sample }).unwrap();
    }
    wtr.flush().unwrap();
}
fn calculate_w0_command(args: CalculateW0Args) {
    let wf =
        load_wf_observables_from_file(&args.wf.wf_filename).thermalise(args.wf.wf_thermalisation);
    let results = (0..args.boot.n_boot)
        .into_par_iter()
        .map(|_| {
            let samples = get_samples(wf.t2_esym.nconfs, args.boot.binwidth);
            calculate_w0(
                calculate_w(
                    &wf.get_subsample_mean_stderr_from_samples(
                        samples,
                        WilsonFlowObservables::T2Esym,
                    )
                    .values,
                    &wf.t,
                ),
                args.wf.w_ref,
            )
        })
        .collect::<Vec<f64>>();
    let mut wtr = csv::Writer::from_writer(stdout());
    for sample in results {
        wtr.serialize(BootstrapSample { sample }).unwrap();
    }
    wtr.flush().unwrap();
}

fn histogram_command(args: HistogramArgs) {
    let mut sample: Vec<f64> = vec![];
    let mut rdr = csv::Reader::from_reader(File::open(args.csv_filename).unwrap());
    for result in rdr.deserialize() {
        let record: BootstrapSample = result.unwrap();
        sample.push(record.sample);
    }
    sample.sort_by(f64::total_cmp);
    let hist = bin(&sample, args.nbins);
    let mut wtr = csv::Writer::from_writer(stdout());
    for hist_row in hist {
        wtr.serialize(hist_row).unwrap();
    }
    wtr.flush().unwrap();
}

fn bootstrap_error_command(args: BootstrapErrorArgs) {
    let mut sample: Vec<f64> = vec![];
    let mut rdr = csv::Reader::from_reader(File::open(args.csv_filename).unwrap());
    for result in rdr.deserialize() {
        let record: BootstrapSample = result.unwrap();
        sample.push(record.sample);
    }
    let results = (0..args.n_boot)
        .into_par_iter()
        .map(|_| {
            let mut tmp = vec![];
            for index in get_samples(sample.len(), 1) {
                tmp.push(sample[index]);
            }
            standard_error(&tmp)
        })
        .collect::<Vec<f64>>();
    let mut wtr = csv::Writer::from_writer(stdout());
    for sample in results {
        wtr.serialize(BootstrapSample { sample }).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn parser() {
    let app = App::parse();
    match app.command {
        Command::ComputeEffectiveMass { args } => compute_effective_mass_command(args),
        Command::FitEffectiveMass { args } => fit_effective_mass_command(args),
        Command::BootstrapFitsWithWF { args } => bootstrap_fits_with_wf_command(args),
        Command::BootstrapFits { args } => bootstrap_fits_command(args),
        Command::BootstrapFitsRatio { args } => bootstrap_fits_ratio_command(args),
        Command::CalculateW0 { args } => calculate_w0_command(args),
        Command::Histogram { args } => histogram_command(args),
        Command::BootstrapError { args } => bootstrap_error_command(args),
        Command::GenerateCompletions {} => {
            generate(Nushell, &mut App::command(), "reshotka", &mut stdout())
        }
    }
}
