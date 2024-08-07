import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class CpuDisplay extends StatelessWidget {
  const CpuDisplay({super.key, required this.device});

  final Device device;

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
                children: [
                  const Text("CPU1"),
                  const Text("CPU2"),
                ],
              ),
            ],
          )),
    );
  }
}
