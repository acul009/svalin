import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';

class CpuDisplay extends StatelessWidget {
  const CpuDisplay({super.key, required this.cpuStatus});

  final CpuStatus cpuStatus;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            children: [
              const Text(style: TextStyle(fontSize: 20), "CPU"),
              const SizedBox(height: 10),
              Wrap(
                children: cpuStatus.cores.map(
                  (element) {
                    return CircularProgressIndicator(
                      value: element.load,
                    );
                  },
                ).toList(),
              ),
            ],
          )),
    );
  }
}
