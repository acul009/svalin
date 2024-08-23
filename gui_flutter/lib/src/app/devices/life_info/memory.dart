import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';

class MemoryDisplay extends StatelessWidget {
  const MemoryDisplay(
      {super.key, required this.memoryStatus, required this.swapStatus});

  final MemoryStatus memoryStatus;
  final SwapStatus swapStatus;

  @override
  Widget build(BuildContext context) {
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
              "Memory",
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
                  "RAM Usage",
                  style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 5),
                ClipRRect(
                  borderRadius: BorderRadius.circular(10),
                  child: LinearProgressIndicator(
                    value: memoryStatus.used / memoryStatus.total,
                    minHeight: 10,
                    backgroundColor: Colors.grey[300],
                    color: Colors.blue,
                  ),
                ),
                const SizedBox(height: 5),
                Text(
                  "${(memoryStatus.used / BigInt.from(1024 * 1024 * 1024)).toStringAsFixed(1)} / ${(memoryStatus.total / BigInt.from(1024 * 1024 * 1024)).toStringAsFixed(1)} GiB",
                  style: const TextStyle(
                      fontSize: 14, fontWeight: FontWeight.w500),
                  textAlign: TextAlign.end,
                ),
              ],
            ),
            const SizedBox(height: 20),
            Builder(builder: (context) {
              if (swapStatus.total > BigInt.zero) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      "SWAP Usage",
                      style:
                          TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                    ),
                    const SizedBox(height: 5),
                    ClipRRect(
                      borderRadius: BorderRadius.circular(10),
                      child: LinearProgressIndicator(
                        value: swapStatus.used / swapStatus.total,
                        minHeight: 10,
                        backgroundColor: Colors.grey[300],
                        color: Colors.blue,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      "${(swapStatus.used / BigInt.from(1024 * 1024 * 1024)).toStringAsFixed(1)} / ${(swapStatus.total / BigInt.from(1024 * 1024 * 1024)).toStringAsFixed(1)} GiB",
                      style: const TextStyle(
                          fontSize: 14, fontWeight: FontWeight.w500),
                      textAlign: TextAlign.end,
                    ),
                  ],
                );
              }
              return const Text(
                "No Swap configured",
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
              );
            }),
          ],
        ),
      ),
    );
  }
}
