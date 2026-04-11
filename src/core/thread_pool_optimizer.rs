use rayon::ThreadPool;
use rayon::ThreadPoolBuildError;
use std::sync::Arc;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;

/// CPU affinity configuration for thread optimization
#[derive(Debug, Clone)]
pub struct CpuAffinityConfig {
    pub enable_affinity: bool,
    pub enable_numa_awareness: bool,
    pub core_ids: Option<Vec<usize>>,
    pub numa_nodes: Option<Vec<usize>>,
}

impl Default for CpuAffinityConfig {
    fn default() -> Self {
        Self {
            enable_affinity: true,
            enable_numa_awareness: false, // Disabled by default for compatibility
            core_ids: None,
            numa_nodes: None,
        }
    }
}

/// NUMA node information for memory allocation optimization
#[derive(Debug, Clone)]
pub struct NumaNode {
    pub node_id: usize,
    pub cpu_cores: Vec<usize>,
    pub memory_size_mb: u64,
    pub distance: Vec<u64>, // Distance to other NUMA nodes
}

/// Enhanced thread pool with CPU affinity and NUMA optimization
pub struct OptimizedThreadPool {
    pool: Arc<ThreadPool>,
    config: CpuAffinityConfig,
    numa_topology: Option<Vec<NumaNode>>,
    thread_assignments: Arc<std::sync::RwLock<HashMap<usize, ThreadInfo>>>,
}

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub thread_id: usize,
    pub cpu_core: Option<usize>,
    pub numa_node: Option<usize>,
    pub created_at: std::time::Instant,
}

impl OptimizedThreadPool {
    pub fn new(num_threads: Option<usize>, config: CpuAffinityConfig) -> Result<Self, ThreadPoolBuildError> {
        let num_threads = num_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or_else(|_| rayon::current_num_threads())
        });
        
        info!("Creating optimized thread pool with {} threads", num_threads);
        
        // Detect NUMA topology if enabled
        let numa_topology = if config.enable_numa_awareness {
            Self::detect_numa_topology()
        } else {
            None
        };
        
        let thread_assignments = Arc::new(std::sync::RwLock::new(HashMap::new()));
        
        // Build thread pool with custom configuration
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("sol-worker-{}", i))
            .spawn_handler(|thread| {
                let thread_id = thread.index();
                
                // Set CPU affinity if enabled
                if config.enable_affinity {
                    if let Some(core_id) = Self::assign_cpu_core(thread_id, &config, &numa_topology) {
                        if let Err(e) = Self::set_thread_affinity(core_id) {
                            error!("Failed to set CPU affinity for thread {}: {}", thread_id, e);
                            // Don't fail thread creation, just continue without affinity
                        } else {
                            info!("Thread {} assigned to CPU core {}", thread_id, core_id);
                        }
                    }
                }
                
                // Store thread assignment information
                let thread_info = ThreadInfo {
                    thread_id,
                    cpu_core: Self::get_assigned_core(thread_id, &config, &numa_topology),
                    numa_node: Self::get_numa_node(thread_id, &numa_topology),
                    created_at: std::time::Instant::now(),
                };
                
                // Note: This would need to be made thread-safe in a real implementation
                debug!("Created thread: {:?}", thread_info);
                
                thread.run();
                Ok(())
            })
            .build()?;
        
        Ok(Self {
            pool: Arc::new(pool),
            config,
            numa_topology,
            thread_assignments,
        })
    }
    
    pub fn pool(&self) -> Arc<ThreadPool> {
        Arc::clone(&self.pool)
    }
    
    pub fn config(&self) -> &CpuAffinityConfig {
        &self.config
    }
    
    pub fn get_numa_topology(&self) -> Option<&Vec<NumaNode>> {
        self.numa_topology.as_ref()
    }
    
    pub fn get_thread_assignments(&self) -> std::sync::RwLockReadGuard<'_, HashMap<usize, ThreadInfo>> {
        self.thread_assignments.read().unwrap()
    }
    
    /// Detect NUMA topology on the current system
    fn detect_numa_topology() -> Option<Vec<NumaNode>> {
        #[cfg(target_os = "linux")]
        {
            // On Linux, we can read /sys/devices/system/node/
            let mut nodes = Vec::new();
            
            if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.file_name().unwrap_or_default().to_str().unwrap_or("").starts_with("node") {
                        if let Some(node_str) = path.file_name().and_then(|n| n.to_str()) {
                            if let Ok(node_id) = node_str.strip_prefix("node").unwrap_or("0").parse::<usize>() {
                                let cpu_cores = Self::get_numa_node_cores(node_id);
                                let memory_size = Self::get_numa_node_memory(node_id);
                                
                                nodes.push(NumaNode {
                                    node_id,
                                    cpu_cores,
                                    memory_size_mb: memory_size,
                                    distance: Self::get_numa_distances(node_id),
                                });
                            }
                        }
                    }
                }
            }
            
            if !nodes.is_empty() {
                info!("Detected {} NUMA nodes", nodes.len());
                Some(nodes)
            } else {
                debug!("No NUMA nodes detected, falling back to UMA");
                None
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            debug!("NUMA detection not supported on this platform");
            None
        }
    }
    
    #[cfg(target_os = "linux")]
    fn get_numa_node_cores(node_id: usize) -> Vec<usize> {
        let mut cores = Vec::new();
        let node_path = format!("/sys/devices/system/node/node{}/cpulist", node_id);
        
        if let Ok(content) = std::fs::read_to_string(node_path) {
            // Parse cpulist format like "0-7,16-23"
            for part in content.trim().split(',') {
                if part.contains('-') {
                    let mut range = part.split('-');
                    if let (Some(start), Some(end)) = (range.next(), range.next()) {
                        if let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) {
                            cores.extend(start..=end);
                        }
                    }
                } else if let Ok(core) = part.parse::<usize>() {
                    cores.push(core);
                }
            }
        }
        
        cores
    }
    
    #[cfg(target_os = "linux")]
    fn get_numa_node_memory(node_id: usize) -> u64 {
        let node_path = format!("/sys/devices/system/node/node{}/meminfo", node_id);
        
        if let Ok(content) = std::fs::read_to_string(node_path) {
            for line in content.lines() {
                if line.contains("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(3) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb / 1024; // Convert to MB
                        }
                    }
                }
            }
        }
        
        0 // Default if not found
    }
    
    #[cfg(target_os = "linux")]
    fn get_numa_distances(_node_id: usize) -> Vec<u64> {
        // Simplified - in a real implementation, this would read from /sys/devices/system/node/nodeX/distance
        vec![10, 20, 30, 40] // Example distances
    }
    
    /// Assign a CPU core to a thread based on configuration
    fn assign_cpu_core(thread_id: usize, config: &CpuAffinityConfig, numa_topology: &Option<Vec<NumaNode>>) -> Option<usize> {
        if let Some(ref core_ids) = config.core_ids {
            // Use explicitly provided core IDs
            return core_ids.get(thread_id % core_ids.len()).copied();
        }
        
        if let Some(ref numa_nodes) = numa_topology {
            // Use NUMA-aware assignment
            let node_id = thread_id % numa_nodes.len();
            let node = &numa_nodes[node_id];
            
            if !node.cpu_cores.is_empty() {
                let core_index = thread_id % node.cpu_cores.len();
                return Some(node.cpu_cores[core_index]);
            }
        }
        
        // Default: assign thread_id as core ID (may not be optimal but works)
        Some(thread_id)
    }
    
    fn get_assigned_core(thread_id: usize, config: &CpuAffinityConfig, numa_topology: &Option<Vec<NumaNode>>) -> Option<usize> {
        Self::assign_cpu_core(thread_id, config, numa_topology)
    }
    
    fn get_numa_node(thread_id: usize, numa_topology: &Option<Vec<NumaNode>>) -> Option<usize> {
        if let Some(ref nodes) = numa_topology {
            Some(thread_id % nodes.len())
        } else {
            None
        }
    }
    
    /// Set CPU affinity for the current thread
    fn set_thread_affinity(_core_id: usize) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            use libc::{cpu_set_t, sched_setaffinity, sched_getaffinity};
            use std::mem;
            
            unsafe {
                let mut cpuset: cpu_set_t = mem::zeroed();
                libc::CPU_ZERO(&mut cpuset);
                libc::CPU_SET(_core_id, &mut cpuset);
                
                let result = sched_setaffinity(0, mem::size_of::<cpu_set_t>(), &cpuset);
                if result == 0 {
                    Ok(())
                } else {
                    Err(format!("Failed to set CPU affinity: {}", std::io::Error::last_os_error()))
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("CPU affinity not supported on this platform");
            Ok(())
        }
    }
    
    /// Get current thread's CPU affinity
    pub fn get_thread_affinity() -> Result<Vec<usize>, String> {
        #[cfg(target_os = "linux")]
        {
            use libc::{cpu_set_t, sched_getaffinity};
            use std::mem;
            
            unsafe {
                let mut cpuset: cpu_set_t = mem::zeroed();
                let result = sched_getaffinity(0, mem::size_of::<cpu_set_t>(), &mut cpuset);
                
                if result == 0 {
                    let mut cores = Vec::new();
                    for i in 0..libc::CPU_SETSIZE as usize {
                        if libc::CPU_ISSET(i, &cpuset) {
                            cores.push(i);
                        }
                    }
                    Ok(cores)
                } else {
                    Err(format!("Failed to get CPU affinity: {}", std::io::Error::last_os_error()))
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Ok(vec![]) // Not supported
        }
    }
    
    /// Optimize memory allocation for NUMA
    pub fn optimize_numa_memory(&self) {
        if let Some(ref numa_nodes) = self.numa_topology {
            info!("Optimizing memory allocation for {} NUMA nodes", numa_nodes.len());
            
            // In a real implementation, this would:
            // 1. Set memory allocation policies
            // 2. Configure thread-local memory pools per NUMA node
            // 3. Use numa_alloc_onnode() for critical allocations
            
            for node in numa_nodes {
                debug!("NUMA Node {}: {} cores, {} MB memory", 
                       node.node_id, node.cpu_cores.len(), node.memory_size_mb);
            }
        }
    }
    
    /// Get performance metrics for the thread pool
    pub fn get_metrics(&self) -> ThreadPoolMetrics {
        ThreadPoolMetrics {
            total_threads: self.pool.current_num_threads(),
            active_threads: self.pool.current_thread_index().map_or(0, |i| i + 1),
            cpu_affinity_enabled: self.config.enable_affinity,
            numa_awareness_enabled: self.config.enable_numa_awareness,
            numa_nodes: self.numa_topology.as_ref().map(|nodes| nodes.len()),
            thread_assignments: self.thread_assignments.read().unwrap().len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadPoolMetrics {
    pub total_threads: usize,
    pub active_threads: usize,
    pub cpu_affinity_enabled: bool,
    pub numa_awareness_enabled: bool,
    pub numa_nodes: Option<usize>,
    pub thread_assignments: usize,
}

/// Builder for optimized thread pools
pub struct OptimizedThreadPoolBuilder {
    num_threads: Option<usize>,
    config: CpuAffinityConfig,
}

impl OptimizedThreadPoolBuilder {
    pub fn new() -> Self {
        Self {
            num_threads: None,
            config: CpuAffinityConfig::default(),
        }
    }
    
    pub fn num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = Some(num_threads);
        self
    }
    
    pub fn enable_cpu_affinity(mut self, enable: bool) -> Self {
        self.config.enable_affinity = enable;
        self
    }
    
    pub fn enable_numa_awareness(mut self, enable: bool) -> Self {
        self.config.enable_numa_awareness = enable;
        self
    }
    
    pub fn core_ids(mut self, cores: Vec<usize>) -> Self {
        self.config.core_ids = Some(cores);
        self
    }
    
    pub fn numa_nodes(mut self, nodes: Vec<usize>) -> Self {
        self.config.numa_nodes = Some(nodes);
        self
    }
    
    pub fn build(self) -> Result<OptimizedThreadPool, ThreadPoolBuildError> {
        OptimizedThreadPool::new(self.num_threads, self.config)
    }
}

impl Default for OptimizedThreadPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_thread_pool_builder() {
        let builder = OptimizedThreadPoolBuilder::new()
            .num_threads(4)
            .enable_cpu_affinity(true)
            .enable_numa_awareness(false);
        
        assert!(builder.build().is_ok());
    }
    
    #[test]
    fn test_cpu_affinity_detection() {
        // This test may fail on systems without proper permissions
        // but should not panic
        let _affinity = OptimizedThreadPool::get_thread_affinity();
    }
    
    #[test]
    fn test_numa_detection() {
        let _topology = OptimizedThreadPool::detect_numa_topology();
        // Should not panic, may return None on non-NUMA systems
    }
}
