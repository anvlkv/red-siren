pub struct RuntimeSubsystem {
    dsp_net_frontend: RwLock<Option<Net>>,
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    gain_param: Arc<Shared>,
    
    processed_output_snoops: RwLock<Option<(Snoop, Snoop)>>,
    input_snoop: RwLock<Option<Snoop>>,
    node_excitement_snoops: RwLock<HashMap<NodeKey, (Snoop, Snoop)>>,
    node_output_snoops: RwLock<HashMap<NodeKey, Snoop>>,
    
    preset: RwLock<Preset>,
    node_band_controls: RwLock<HashMap<NodeKey, Shared>>,
    node_key_controls: RwLock<HashMap<NodeKey, Shared>>,
    node_sensor_controls: RwLock<HashMap<NodeKey, SensorHandles>>,
    siren_excitements: RwLock<HashMap<NodeKey, ExcitementControl>>,
    
    tuner_tap_gain_param: RwLock<Option<Shared>>,
    tuner_freq_range: Arc<(Shared, Shared)>,
    tuner_ny_threshold: Arc<Shared>,
    tuner_ny_wet_ratio: Arc<Shared>,
    spectrum_data_thb: SpectrumBuffer,
}