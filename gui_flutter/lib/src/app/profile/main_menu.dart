import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class MainMenu extends StatelessWidget {
  const MainMenu({super.key, required this.client});

  final Client client;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Svalin")),
      body: Center(child: Text("This is the not yet implemented main menu.")),
    );
  }
}
