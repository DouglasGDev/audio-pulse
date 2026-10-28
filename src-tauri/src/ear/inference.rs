//! ONNX Runtime inference for the local monocular depth model.

use ort::environment::Environment;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use parking_lot::Mutex;
use std::sync::OnceLock;

use super::geometry::DepthMap;
use super::model;

pub const INPUT_SIZE: usize = 518;

static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn environment() -> Result<Environment, String> {
    ENV.get_or_init(|| {
        ort::init()
            .with_name("Audio Pulse Ear Profile")
            .build()
            .map_err(|err| format!("Falha ao iniciar o ONNX Runtime: {err}"))
    })
    .clone()
}

fn build_session() -> Result<Session, String> {
    let env = environment()?;
    let path = model::model_path();
    if !path.exists() {
        return Err("O modelo de profundidade ainda não foi baixado.".into());
    }
    // Two threads keep the one-shot analysis responsive without competing with
    // the audio callback on machines with few cores.
    let threads = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .saturating_sub(1)
        .clamp(1, 4);
    let session = Session::builder(&env)
        .map_err(|err| format!("Falha ao criar a sessão ONNX: {err}"))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|err| format!("Falha ao configurar o modelo: {err}"))?
        .with_intra_threads(threads)
        .map_err(|err| format!("Falha ao configurar o modelo: {err}"))?
        .commit_from_file(&path)
        .map_err(|err| format!("Não foi possível carregar o modelo de profundidade: {err}"))?;
    log::info!(
        "depth model loaded ({} threads): inputs {:?}, outputs {:?}",
        threads,
        session.inputs().iter().map(|input| input.name()).collect::<Vec<_>>(),
        session.outputs().iter().map(|output| output.name()).collect::<Vec<_>>()
    );
    Ok(session)
}

/// Runs the model on a normalized NCHW tensor and returns the raw inverse-depth
/// map. Larger values mean closer to the camera.
pub fn run(input: Vec<f32>) -> Result<DepthMap, String> {
    let expected = 3 * INPUT_SIZE * INPUT_SIZE;
    if input.len() != expected {
        return Err("Entrada do modelo com tamanho inesperado.".into());
    }
    let tensor = Tensor::from_array(([1usize, 3, INPUT_SIZE, INPUT_SIZE], input))
        .map_err(|err| format!("Falha ao preparar a entrada do modelo: {err}"))?;

    let mut guard = SESSION.lock();
    if guard.is_none() {
        *guard = Some(build_session()?);
    }
    let session = guard.as_mut().expect("session initialized above");
    let outputs = session
        .run(ort::inputs![tensor])
        .map_err(|err| format!("Falha na inferência de profundidade: {err}"))?;
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|err| format!("Saída do modelo inesperada: {err}"))?;
    let dims: Vec<i64> = shape.iter().copied().collect();
    if dims.len() < 2 {
        return Err("Saída do modelo inesperada.".into());
    }
    let height = dims[dims.len() - 2].max(1) as usize;
    let width = dims[dims.len() - 1].max(1) as usize;
    if width * height > data.len() {
        return Err("Saída do modelo inesperada.".into());
    }
    let mut depth = Vec::with_capacity(width * height);
    for value in data.iter().take(width * height) {
        depth.push(if value.is_finite() { *value } else { 0.0 });
    }
    Ok(DepthMap {
        width,
        height,
        data: depth,
    })
}
