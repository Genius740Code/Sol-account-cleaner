use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::{info, warn};
use std::collections::VecDeque;

/// Comprehensive system resource monitoring
#[derive(Debug, Clone)]
pub struct SystemResourceMonitor {
    cpu_monitor: Arc<CpuMonitor>,
    memory_monitor: Arc<MemoryMonitor>,
    network_monitor: Arc<NetworkMonitor>,
    disk_monitor: Arc<DiskMonitor>,
    process_monitor: Arc<ProcessMonitor>,
    metrics_history: Arc<RwLock<VecDeque<ResourceSnapshot>>>,
    config: MonitorConfig,
}

/// Configuration for resource monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub sampling_interval_ms: u64,
    pub history_size: usize,
    pub enable_cpu_monitoring: bool,
    pub enable_memory_monitoring: bool,
    pub enable_network_monitoring: bool,
    pub enable_disk_monitoring: bool,
    pub enable_process_monitoring: bool,
    pub alert_thresholds: AlertThresholds,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 1000, // 1 second
            history_size: 3600, // Keep 1 hour of history at 1-second intervals
            enable_cpu_monitoring: true,
            enable_memory_monitoring: true,
            enable_network_monitoring: true,
            enable_disk_monitoring: false, // Disabled by default
            enable_process_monitoring: true,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub network_rps: u64,
    pub disk_usage_percent: f64,
    pub process_count: usize,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 80.0,
            memory_usage_percent: 85.0,
            network_rps: 10000,
            disk_usage_percent: 90.0,
            process_count: 1000,
        }
    }
}

/// CPU monitoring with per-core metrics
#[derive(Debug)]
pub struct CpuMonitor {
    total_usage: AtomicU64, // Percentage * 100
    core_usage: Vec<AtomicU64>,
    load_average: AtomicU64, // Load average * 100
    context_switches: AtomicU64,
    last_update: AtomicU64,
}

impl CpuMonitor {
    pub fn new(num_cores: usize) -> Self {
        let mut core_usage = Vec::with_capacity(num_cores);
        for _ in 0..num_cores {
            core_usage.push(AtomicU64::new(0));
        }
        
        Self {
            total_usage: AtomicU64::new(0),
            core_usage,
            load_average: AtomicU64::new(0),
            context_switches: AtomicU64::new(0),
            last_update: AtomicU64::new(0),
        }
    }
    
    pub fn update(&self) -> CpuMetrics {
        let metrics = self.collect_cpu_metrics();
        
        self.total_usage.store((metrics.total_usage * 100.0) as u64, Ordering::Relaxed);
        self.load_average.store((metrics.load_average * 100.0) as u64, Ordering::Relaxed);
        self.context_switches.store(metrics.context_switches, Ordering::Relaxed);
        self.last_update.store(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(), Ordering::Relaxed);
        
        // Update per-core usage
        for (i, &usage) in metrics.core_usage.iter().enumerate() {
            if i < self.core_usage.len() {
                self.core_usage[i].store((usage * 100.0) as u64, Ordering::Relaxed);
            }
        }
        
        metrics
    }
    
    fn collect_cpu_metrics(&self) -> CpuMetrics {
        #[cfg(target_os = "linux")]
        {
            self.collect_linux_cpu_metrics()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            self.collect_generic_cpu_metrics()
        }
    }
    
    #[cfg(target_os = "linux")]
    fn collect_linux_cpu_metrics(&self) -> CpuMetrics {
        let mut metrics = CpuMetrics::default();
        
        // Read /proc/stat for CPU usage
        if let Ok(stat_content) = std::fs::read_to_string("/proc/stat") {
            if let Some(cpu_line) = stat_content.lines().next() {
                let parts: Vec<&str> = cpu_line.split_whitespace().collect();
                if parts.len() >= 8 && parts[0] == "cpu" {
                    let times: Vec<u64> = parts[1..9].iter().filter_map(|s| s.parse().ok()).collect();
                    if times.len() == 8 {
                        let total = times.iter().sum::<u64>();
                        let idle = times[3] + times[4];
                        let usage = if total > 0 {
                            (1.0 - (idle as f64 / total as f64)) * 100.0
                        } else {
                            0.0
                        };
                        metrics.total_usage = usage;
                    }
                }
            }
        }
        
        // Read /proc/loadavg for load average
        if let Ok(loadavg_content) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(load_part) = loadavg_content.split_whitespace().next() {
                if let Ok(load) = load_part.parse::<f64>() {
                    metrics.load_average = load;
                }
            }
        }
        
        // Read /proc/stat for context switches
        if let Ok(stat_content) = std::fs::read_to_string("/proc/stat") {
            for line in stat_content.lines() {
                if line.starts_with("ctxt ") {
                    if let Some(count) = line.split_whitespace().nth(1) {
                        if let Ok(switches) = count.parse::<u64>() {
                            metrics.context_switches = switches;
                        }
                    }
                    break;
                }
            }
        }
        
        metrics
    }
    
    #[cfg(not(target_os = "linux"))]
    fn collect_generic_cpu_metrics(&self) -> CpuMetrics {
        // Fallback implementation for non-Linux systems
        CpuMetrics::default()
    }
    
    pub fn get_metrics(&self) -> CpuMetrics {
        CpuMetrics {
            total_usage: self.total_usage.load(Ordering::Relaxed) as f64 / 100.0,
            core_usage: self.core_usage.iter().map(|u| u.load(Ordering::Relaxed) as f64 / 100.0).collect(),
            load_average: self.load_average.load(Ordering::Relaxed) as f64 / 100.0,
            context_switches: self.context_switches.load(Ordering::Relaxed),
            last_update: self.last_update.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CpuMetrics {
    pub total_usage: f64,
    pub core_usage: Vec<f64>,
    pub load_average: f64,
    pub context_switches: u64,
    pub last_update: u64,
}

/// Memory monitoring with detailed breakdown
#[derive(Debug)]
pub struct MemoryMonitor {
    total_memory_mb: AtomicU64,
    used_memory_mb: AtomicU64,
    available_memory_mb: AtomicU64,
    swap_total_mb: AtomicU64,
    swap_used_mb: AtomicU64,
    cache_memory_mb: AtomicU64,
    buffers_memory_mb: AtomicU64,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        Self {
            total_memory_mb: AtomicU64::new(0),
            used_memory_mb: AtomicU64::new(0),
            available_memory_mb: AtomicU64::new(0),
            swap_total_mb: AtomicU64::new(0),
            swap_used_mb: AtomicU64::new(0),
            cache_memory_mb: AtomicU64::new(0),
            buffers_memory_mb: AtomicU64::new(0),
        }
    }
    
    pub fn update(&self) -> MemoryMetrics {
        let metrics = self.collect_memory_metrics();
        
        self.total_memory_mb.store(metrics.total_memory_mb, Ordering::Relaxed);
        self.used_memory_mb.store(metrics.used_memory_mb, Ordering::Relaxed);
        self.available_memory_mb.store(metrics.available_memory_mb, Ordering::Relaxed);
        self.swap_total_mb.store(metrics.swap_total_mb, Ordering::Relaxed);
        self.swap_used_mb.store(metrics.swap_used_mb, Ordering::Relaxed);
        self.cache_memory_mb.store(metrics.cache_memory_mb, Ordering::Relaxed);
        self.buffers_memory_mb.store(metrics.buffers_memory_mb, Ordering::Relaxed);
        
        metrics
    }
    
    fn collect_memory_metrics(&self) -> MemoryMetrics {
        #[cfg(target_os = "linux")]
        {
            self.collect_linux_memory_metrics()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            self.collect_generic_memory_metrics()
        }
    }
    
    #[cfg(target_os = "linux")]
    fn collect_linux_memory_metrics(&self) -> MemoryMetrics {
        let mut metrics = MemoryMetrics::default();
        
        if let Ok(meminfo_content) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Some(value) = parts[1].parse::<u64>().ok() {
                        let kb_to_mb = value / 1024;
                        
                        match parts[0] {
                            "MemTotal:" => metrics.total_memory_mb = kb_to_mb,
                            "MemFree:" => metrics.free_memory_mb = kb_to_mb,
                            "MemAvailable:" => metrics.available_memory_mb = kb_to_mb,
                            "Buffers:" => metrics.buffers_memory_mb = kb_to_mb,
                            "Cached:" => metrics.cache_memory_mb = kb_to_mb,
                            "SwapTotal:" => metrics.swap_total_mb = kb_to_mb,
                            "SwapFree:" => metrics.swap_free_mb = kb_to_mb,
                            _ => {}
                        }
                    }
                }
            }
            
            // Calculate used memory
            metrics.used_memory_mb = metrics.total_memory_mb.saturating_sub(metrics.available_memory_mb);
            metrics.swap_used_mb = metrics.swap_total_mb.saturating_sub(metrics.swap_free_mb);
        }
        
        metrics
    }
    
    #[cfg(not(target_os = "linux"))]
    fn collect_generic_memory_metrics(&self) -> MemoryMetrics {
        // Fallback implementation using sysinfo crate or similar
        MemoryMetrics::default()
    }
    
    pub fn get_metrics(&self) -> MemoryMetrics {
        MemoryMetrics {
            total_memory_mb: self.total_memory_mb.load(Ordering::Relaxed),
            used_memory_mb: self.used_memory_mb.load(Ordering::Relaxed),
            available_memory_mb: self.available_memory_mb.load(Ordering::Relaxed),
            free_memory_mb: self.available_memory_mb.load(Ordering::Relaxed).saturating_sub(
                self.used_memory_mb.load(Ordering::Relaxed)
            ),
            swap_total_mb: self.swap_total_mb.load(Ordering::Relaxed),
            swap_used_mb: self.swap_used_mb.load(Ordering::Relaxed),
            swap_free_mb: self.swap_total_mb.load(Ordering::Relaxed).saturating_sub(
                self.swap_used_mb.load(Ordering::Relaxed)
            ),
            cache_memory_mb: self.cache_memory_mb.load(Ordering::Relaxed),
            buffers_memory_mb: self.buffers_memory_mb.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MemoryMetrics {
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub available_memory_mb: u64,
    pub free_memory_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub swap_free_mb: u64,
    pub cache_memory_mb: u64,
    pub buffers_memory_mb: u64,
}

/// Network monitoring with interface-specific metrics
#[derive(Debug)]
pub struct NetworkMonitor {
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    connections_active: AtomicUsize,
    connections_established: AtomicUsize,
    requests_per_second: AtomicU64,
    last_update: AtomicU64,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            connections_active: AtomicUsize::new(0),
            connections_established: AtomicUsize::new(0),
            requests_per_second: AtomicU64::new(0),
            last_update: AtomicU64::new(0),
        }
    }
    
    pub fn update(&self) -> NetworkMetrics {
        let metrics = self.collect_network_metrics();
        
        self.bytes_sent.store(metrics.bytes_sent, Ordering::Relaxed);
        self.bytes_received.store(metrics.bytes_received, Ordering::Relaxed);
        self.packets_sent.store(metrics.packets_sent, Ordering::Relaxed);
        self.packets_received.store(metrics.packets_received, Ordering::Relaxed);
        self.connections_active.store(metrics.connections_active, Ordering::Relaxed);
        self.connections_established.store(metrics.connections_established, Ordering::Relaxed);
        self.requests_per_second.store(metrics.requests_per_second, Ordering::Relaxed);
        self.last_update.store(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(), Ordering::Relaxed);
        
        metrics
    }
    
    fn collect_network_metrics(&self) -> NetworkMetrics {
        #[cfg(target_os = "linux")]
        {
            self.collect_linux_network_metrics()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            self.collect_generic_network_metrics()
        }
    }
    
    #[cfg(target_os = "linux")]
    fn collect_linux_network_metrics(&self) -> NetworkMetrics {
        let mut metrics = NetworkMetrics::default();
        
        // Read /proc/net/dev for network interface statistics
        if let Ok(dev_content) = std::fs::read_to_string("/proc/net/dev") {
            for line in dev_content.lines().skip(2) { // Skip header lines
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 17 {
                    // Skip loopback interface
                    if parts[0] != "lo:" {
                        if let (Ok(rx_bytes), Ok(tx_bytes)) = (parts[1].parse::<u64>(), parts[9].parse::<u64>()) {
                            metrics.bytes_received += rx_bytes;
                            metrics.bytes_sent += tx_bytes;
                        }
                        if let (Ok(rx_packets), Ok(tx_packets)) = (parts[2].parse::<u64>(), parts[10].parse::<u64>()) {
                            metrics.packets_received += rx_packets;
                            metrics.packets_sent += tx_packets;
                        }
                    }
                }
            }
        }
        
        // Read /proc/net/tcp and /proc/net/udp for connection counts
        let tcp_connections = std::fs::read_to_string("/proc/net/tcp")
            .map(|content| content.lines().skip(1).count())
            .unwrap_or(0);
        
        let udp_connections = std::fs::read_to_string("/proc/net/udp")
            .map(|content| content.lines().skip(1).count())
            .unwrap_or(0);
        
        metrics.connections_active = tcp_connections + udp_connections;
        metrics.connections_established = tcp_connections; // Approximation
        
        metrics
    }
    
    #[cfg(not(target_os = "linux"))]
    fn collect_generic_network_metrics(&self) -> NetworkMetrics {
        NetworkMetrics::default()
    }
    
    pub fn get_metrics(&self) -> NetworkMetrics {
        NetworkMetrics {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            connections_active: self.connections_active.load(Ordering::Relaxed),
            connections_established: self.connections_established.load(Ordering::Relaxed),
            requests_per_second: self.requests_per_second.load(Ordering::Relaxed),
            last_update: self.last_update.load(Ordering::Relaxed),
        }
    }
    
    pub fn increment_requests(&self) {
        self.requests_per_second.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connections_active: usize,
    pub connections_established: usize,
    pub requests_per_second: u64,
    pub last_update: u64,
}

/// Disk monitoring (basic implementation)
#[derive(Debug)]
pub struct DiskMonitor {
    total_space_gb: AtomicU64,
    used_space_gb: AtomicU64,
    available_space_gb: AtomicU64,
    read_ops: AtomicU64,
    write_ops: AtomicU64,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            total_space_gb: AtomicU64::new(0),
            used_space_gb: AtomicU64::new(0),
            available_space_gb: AtomicU64::new(0),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
        }
    }
    
    pub fn update(&self) -> DiskMetrics {
        let metrics = self.collect_disk_metrics();
        
        self.total_space_gb.store(metrics.total_space_gb, Ordering::Relaxed);
        self.used_space_gb.store(metrics.used_space_gb, Ordering::Relaxed);
        self.available_space_gb.store(metrics.available_space_gb, Ordering::Relaxed);
        self.read_ops.store(metrics.read_ops, Ordering::Relaxed);
        self.write_ops.store(metrics.write_ops, Ordering::Relaxed);
        
        metrics
    }
    
    fn collect_disk_metrics(&self) -> DiskMetrics {
        // Basic implementation using std::fs
        if let Ok(_metadata) = std::fs::metadata(".") {
            // This is a simplified implementation
            // In production, you'd want to use a proper disk space library
            DiskMetrics::default()
        } else {
            DiskMetrics::default()
        }
    }
    
    pub fn get_metrics(&self) -> DiskMetrics {
        DiskMetrics {
            total_space_gb: self.total_space_gb.load(Ordering::Relaxed),
            used_space_gb: self.used_space_gb.load(Ordering::Relaxed),
            available_space_gb: self.available_space_gb.load(Ordering::Relaxed),
            read_ops: self.read_ops.load(Ordering::Relaxed),
            write_ops: self.write_ops.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskMetrics {
    pub total_space_gb: u64,
    pub used_space_gb: u64,
    pub available_space_gb: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

/// Process monitoring for the current application
#[derive(Debug)]
pub struct ProcessMonitor {
    pid: u32,
    cpu_usage: AtomicU64,
    memory_usage_mb: AtomicU64,
    thread_count: AtomicUsize,
    file_descriptors: AtomicUsize,
    start_time: std::time::Instant,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            pid: std::process::id(),
            cpu_usage: AtomicU64::new(0),
            memory_usage_mb: AtomicU64::new(0),
            thread_count: AtomicUsize::new(0),
            file_descriptors: AtomicUsize::new(0),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn update(&self) -> ProcessMetrics {
        let metrics = self.collect_process_metrics();
        
        self.cpu_usage.store((metrics.cpu_usage * 100.0) as u64, Ordering::Relaxed);
        self.memory_usage_mb.store(metrics.memory_usage_mb, Ordering::Relaxed);
        self.thread_count.store(metrics.thread_count, Ordering::Relaxed);
        self.file_descriptors.store(metrics.file_descriptors, Ordering::Relaxed);
        
        metrics
    }
    
    fn collect_process_metrics(&self) -> ProcessMetrics {
        #[cfg(target_os = "linux")]
        {
            self.collect_linux_process_metrics()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            self.collect_generic_process_metrics()
        }
    }
    
    #[cfg(target_os = "linux")]
    fn collect_linux_process_metrics(&self) -> ProcessMetrics {
        let mut metrics = ProcessMetrics::default();
        
        // Read /proc/self/status for process information
        if let Ok(status_content) = std::fs::read_to_string(format!("/proc/{}/status", self.pid)) {
            for line in status_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    match parts[0] {
                        "VmRSS:" => {
                            if let Some(kb) = parts[1].parse::<u64>().ok() {
                                metrics.memory_usage_mb = kb / 1024;
                            }
                        }
                        "Threads:" => {
                            if let Some(threads) = parts[1].parse::<usize>().ok() {
                                metrics.thread_count = threads;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Count file descriptors
        if let Ok(fd_dir) = std::fs::read_dir(format!("/proc/{}/fd", self.pid)) {
            metrics.file_descriptors = fd_dir.count();
        }
        
        metrics.uptime_seconds = self.start_time.elapsed().as_secs();
        
        metrics
    }
    
    #[cfg(not(target_os = "linux"))]
    fn collect_generic_process_metrics(&self) -> ProcessMetrics {
        ProcessMetrics {
            pid: self.pid,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            ..Default::default()
        }
    }
    
    pub fn get_metrics(&self) -> ProcessMetrics {
        ProcessMetrics {
            pid: self.pid,
            cpu_usage: self.cpu_usage.load(Ordering::Relaxed) as f64 / 100.0,
            memory_usage_mb: self.memory_usage_mb.load(Ordering::Relaxed),
            thread_count: self.thread_count.load(Ordering::Relaxed),
            file_descriptors: self.file_descriptors.load(Ordering::Relaxed),
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub cpu_usage: f64,
    pub memory_usage_mb: u64,
    pub thread_count: usize,
    pub file_descriptors: usize,
    pub uptime_seconds: u64,
}

/// Complete resource snapshot
#[derive(Debug, Clone, Serialize)]
pub struct ResourceSnapshot {
    pub timestamp: u64,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub network: NetworkMetrics,
    pub disk: DiskMetrics,
    pub process: ProcessMetrics,
}

impl SystemResourceMonitor {
    pub fn new(config: MonitorConfig) -> Self {
        let num_cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        
        Self {
            cpu_monitor: Arc::new(CpuMonitor::new(num_cores)),
            memory_monitor: Arc::new(MemoryMonitor::new()),
            network_monitor: Arc::new(NetworkMonitor::new()),
            disk_monitor: Arc::new(DiskMonitor::new()),
            process_monitor: Arc::new(ProcessMonitor::new()),
            metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(config.history_size))),
            config,
        }
    }
    
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting resource monitoring with {}ms interval", self.config.sampling_interval_ms);
        
        let cpu_monitor = Arc::clone(&self.cpu_monitor);
        let memory_monitor = Arc::clone(&self.memory_monitor);
        let network_monitor = Arc::clone(&self.network_monitor);
        let disk_monitor = Arc::clone(&self.disk_monitor);
        let process_monitor = Arc::clone(&self.process_monitor);
        let metrics_history = Arc::clone(&self.metrics_history);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(config.sampling_interval_ms));
            
            loop {
                interval.tick().await;
                
                // Collect all metrics
                let cpu_metrics = cpu_monitor.update();
                let memory_metrics = memory_monitor.update();
                let network_metrics = network_monitor.update();
                let disk_metrics = disk_monitor.update();
                let process_metrics = process_monitor.update();
                
                let snapshot = ResourceSnapshot {
                    timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                    cpu: cpu_metrics,
                    memory: memory_metrics,
                    network: network_metrics,
                    disk: disk_metrics,
                    process: process_metrics,
                };
                
                // Check for alerts before moving
                Self::check_alerts(&snapshot, &config.alert_thresholds).await;
                
                // Add to history (with size limit)
                {
                    let mut history = metrics_history.write().await;
                    if history.len() >= history.capacity() {
                        history.pop_front();
                    }
                    history.push_back(snapshot);
                }
            }
        });
        
        Ok(())
    }
    
    async fn check_alerts(snapshot: &ResourceSnapshot, thresholds: &AlertThresholds) {
        if snapshot.cpu.total_usage > thresholds.cpu_usage_percent {
            warn!("High CPU usage detected: {:.1}%", snapshot.cpu.total_usage);
        }
        
        let memory_usage_percent = if snapshot.memory.total_memory_mb > 0 {
            (snapshot.memory.used_memory_mb as f64 / snapshot.memory.total_memory_mb as f64) * 100.0
        } else {
            0.0
        };
        
        if memory_usage_percent > thresholds.memory_usage_percent {
            warn!("High memory usage detected: {:.1}%", memory_usage_percent);
        }
        
        if snapshot.network.requests_per_second > thresholds.network_rps {
            warn!("High network request rate detected: {} RPS", snapshot.network.requests_per_second);
        }
        
        let disk_usage_percent = if snapshot.disk.total_space_gb > 0 {
            (snapshot.disk.used_space_gb as f64 / snapshot.disk.total_space_gb as f64) * 100.0
        } else {
            0.0
        };
        
        if disk_usage_percent > thresholds.disk_usage_percent {
            warn!("High disk usage detected: {:.1}%", disk_usage_percent);
        }
        
        if snapshot.process.thread_count > thresholds.process_count {
            warn!("High thread count detected: {}", snapshot.process.thread_count);
        }
    }
    
    pub async fn get_current_metrics(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            cpu: self.cpu_monitor.get_metrics(),
            memory: self.memory_monitor.get_metrics(),
            network: self.network_monitor.get_metrics(),
            disk: self.disk_monitor.get_metrics(),
            process: self.process_monitor.get_metrics(),
        }
    }
    
    pub async fn get_metrics_history(&self, duration_secs: Option<u64>) -> Vec<ResourceSnapshot> {
        let history = self.metrics_history.read().await;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        
        if let Some(duration) = duration_secs {
            let cutoff = now - duration;
            history.iter()
                .filter(|snapshot| snapshot.timestamp >= cutoff)
                .cloned()
                .collect()
        } else {
            history.iter().cloned().collect()
        }
    }
    
    pub async fn get_average_metrics(&self, duration_secs: u64) -> Option<ResourceSnapshot> {
        let history = self.get_metrics_history(Some(duration_secs)).await;
        
        if history.is_empty() {
            return None;
        }
        
        let count = history.len() as f64;
        let latest = &history[history.len() - 1];
        
        // Calculate averages (simplified - just averaging CPU and memory)
        let avg_cpu_usage = history.iter().map(|s| s.cpu.total_usage).sum::<f64>() / count;
        let avg_memory_usage = history.iter().map(|s| s.memory.used_memory_mb).sum::<u64>() / count as u64;
        
        Some(ResourceSnapshot {
            timestamp: latest.timestamp,
            cpu: CpuMetrics {
                total_usage: avg_cpu_usage,
                ..latest.cpu.clone()
            },
            memory: MemoryMetrics {
                used_memory_mb: avg_memory_usage,
                ..latest.memory.clone()
            },
            ..latest.clone()
        })
    }
    
    pub fn increment_network_requests(&self) {
        self.network_monitor.increment_requests();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_resource_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = SystemResourceMonitor::new(config);
        
        let metrics = monitor.get_current_metrics().await;
        assert!(metrics.timestamp > 0);
    }
    
    #[tokio::test]
    async fn test_metrics_history() {
        let config = MonitorConfig {
            sampling_interval_ms: 100,
            history_size: 10,
            ..Default::default()
        };
        let monitor = SystemResourceMonitor::new(config);
        
        // Start monitoring briefly
        let _ = monitor.start_monitoring().await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        
        let history = monitor.get_metrics_history(Some(1)).await;
        assert!(!history.is_empty());
    }
}
