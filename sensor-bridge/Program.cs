using System.Text.Json;
using LibreHardwareMonitor.Hardware;

static float? MaxOf(IEnumerable<float> xs) {
    float? m = null;
    foreach (var v in xs) if (m is null || v > m) m = v;
    return m;
}

var computer = new Computer {
    IsCpuEnabled = true,
    IsGpuEnabled = true,
    IsMotherboardEnabled = true,
};

var once = args.Contains("--once");
string? outPath = null;
for (int i = 0; i < args.Length - 1; i++) {
    if (args[i] == "--out") outPath = args[i + 1];
}

using var cts = new CancellationTokenSource();
// Parent can write "quit" to stdin so we Close() and unload R0sensor-bridge.sys.
_ = Task.Run(() => {
    try {
        while (Console.In.ReadLine() is { } line) {
            if (line.Trim().Equals("quit", StringComparison.OrdinalIgnoreCase)) {
                cts.Cancel();
                break;
            }
        }
    } catch {
        // stdin closed / not available
    }
});

TextWriter writer = outPath is null
    ? new StreamWriter(Console.OpenStandardOutput()) { AutoFlush = true }
    : new StreamWriter(outPath, append: false) { AutoFlush = true };

try {
    computer.Open();
    while (!cts.IsCancellationRequested) {
        var cpuTemps = new List<(string name, float v)>();
        var cpuLoads = new List<float>();
        var cpuClocks = new List<float>();
        var cpuPowers = new List<(string name, float v)>();
        var gpuTemps = new List<float>();
        var gpuLoads = new List<float>();
        var gpuClocks = new List<float>();
        var gpuPowers = new List<float>();

        void ingest(IHardware h) {
            h.Update();
            bool isCpu = h.HardwareType == HardwareType.Cpu;
            bool isGpu = h.HardwareType is HardwareType.GpuAmd or HardwareType.GpuNvidia or HardwareType.GpuIntel;
            foreach (var s in h.Sensors) {
                if (!s.Value.HasValue) continue;
                float v = s.Value.Value;
                if (isCpu) {
                    switch (s.SensorType) {
                        case SensorType.Temperature: cpuTemps.Add((s.Name, v)); break;
                        case SensorType.Load:
                            if (s.Name.Contains("Total", StringComparison.OrdinalIgnoreCase) || s.Name.Contains("Max", StringComparison.OrdinalIgnoreCase))
                                cpuLoads.Add(v);
                            break;
                        case SensorType.Clock: cpuClocks.Add(v); break;
                        case SensorType.Power: cpuPowers.Add((s.Name, v)); break;
                    }
                }
                if (isGpu) {
                    switch (s.SensorType) {
                        case SensorType.Temperature: gpuTemps.Add(v); break;
                        case SensorType.Load:
                            if (s.Name.Contains("Core", StringComparison.OrdinalIgnoreCase) || s.Name.Contains("D3D 3D", StringComparison.OrdinalIgnoreCase))
                                gpuLoads.Add(v);
                            break;
                        case SensorType.Clock:
                            if (s.Name.Contains("Core", StringComparison.OrdinalIgnoreCase)) gpuClocks.Add(v);
                            break;
                        case SensorType.Power: gpuPowers.Add(v); break;
                    }
                }
            }
            foreach (var sub in h.SubHardware) ingest(sub);
        }
        foreach (var hw in computer.Hardware) ingest(hw);

        float? pickTemp() {
            foreach (var pref in new[] { "Core Max", "CPU Package", "Tctl", "Tdie", "Package" }) {
                var hit = cpuTemps.FirstOrDefault(t => t.name.Contains(pref, StringComparison.OrdinalIgnoreCase));
                if (hit.name != null) return hit.v;
            }
            return MaxOf(cpuTemps.Select(t => t.v));
        }
        float? pickPower() {
            foreach (var pref in new[] { "Package", "CPU Package", "Cores" }) {
                var hit = cpuPowers.FirstOrDefault(t => t.name.Contains(pref, StringComparison.OrdinalIgnoreCase));
                if (hit.name != null) return hit.v;
            }
            return MaxOf(cpuPowers.Select(t => t.v));
        }

        var payload = new Dictionary<string, object?> {
            ["cpu_temperature"] = pickTemp(),
            ["cpu_load"] = MaxOf(cpuLoads),
            ["cpu_clock"] = MaxOf(cpuClocks),
            ["cpu_power"] = pickPower(),
            ["gpu_temperature"] = MaxOf(gpuTemps),
            ["gpu_load"] = MaxOf(gpuLoads),
            ["gpu_clock"] = MaxOf(gpuClocks),
            ["gpu_power"] = MaxOf(gpuPowers),
            ["cpu_temp_sensors"] = cpuTemps.Count,
            ["cpu_power_sensors"] = cpuPowers.Count,
        };
        writer.WriteLine(JsonSerializer.Serialize(payload));
        if (once) break;
        if (cts.Token.WaitHandle.WaitOne(1000)) break;
    }
} finally {
    // Unloads R0sensor-bridge so sensor-bridge.sys is no longer locked.
    try { computer.Close(); } catch { /* ignore */ }
    writer.Dispose();
}
