import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';

class CpuDisplay extends StatelessWidget {
  const CpuDisplay({super.key, required this.cpuStatus});

  final CpuStatus cpuStatus;

  @override
  Widget build(BuildContext context) {
    // Calculate the average CPU load
    final averageLoad =
        cpuStatus.cores.map((core) => core.load).reduce((a, b) => a + b) /
            cpuStatus.cores.length;

    return Card(
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(15),
      ),
      elevation: 5,
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text(
              "CPU Usage",
              style: TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.bold,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 20),

            // Average CPU Load Bar
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  "Average Load",
                  style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 5),
                ClipRRect(
                  borderRadius: BorderRadius.circular(10),
                  child: LinearProgressIndicator(
                    value: averageLoad / 100,
                    minHeight: 10,
                    backgroundColor: Colors.grey[300],
                    color: Colors.blue,
                  ),
                ),
                const SizedBox(height: 5),
                Text(
                  "${averageLoad.floor()}%",
                  style: const TextStyle(
                      fontSize: 14, fontWeight: FontWeight.w500),
                  textAlign: TextAlign.end,
                ),
              ],
            ),
            const SizedBox(height: 20),

            // CPU Cores
            Wrap(
              alignment: WrapAlignment.center,
              spacing: 20,
              runSpacing: 20,
              children: cpuStatus.cores.asMap().entries.map(
                (entry) {
                  var core = entry.value;
                  return Column(
                    children: [
                      SizedBox(
                        width: 60,
                        height: 60,
                        child: Stack(
                          fit: StackFit.expand,
                          children: [
                            CircularProgressIndicator(
                              value: core.load / 100,
                              strokeWidth: 8,
                              backgroundColor: Colors.grey[300],
                              color: Colors.blue,
                            ),
                            Center(
                              child: Text(
                                "${core.load.floor()}%",
                                style: const TextStyle(
                                    fontSize: 14, fontWeight: FontWeight.bold),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(height: 5),
                      Text(
                        "Core ${entry.key + 1}",
                        style: const TextStyle(fontSize: 14),
                      ),
                    ],
                  );
                },
              ).toList(),
            ),
          ],
        ),
      ),
    );
  }
}
