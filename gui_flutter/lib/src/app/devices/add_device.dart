import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:pin_code_fields/pin_code_fields.dart';

class AddDeviceDialog extends StatefulWidget {
  const AddDeviceDialog({super.key, required this.client});

  final Client client;

  @override
  State<StatefulWidget> createState() {
    return _AddDeviceDialogState();
  }
}

class _AddDeviceDialogState extends State<AddDeviceDialog> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Add Device")),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(50),
          child: Column(
            children: [
              const Text(
                "Input Join Code",
                style: TextStyle(fontSize: 40),
              ),
              SizedBox.fromSize(size: const Size.fromHeight(20)),
              Container(
                constraints: const BoxConstraints(maxWidth: 400),
                child: PinCodeTextField(
                  appContext: context,
                  length: 6,
                  autoFocus: true,
                  showCursor: true,
                  pinTheme: PinTheme(
                    shape: PinCodeFieldShape.box,
                    borderRadius: BorderRadius.circular(5),
                    fieldHeight: 50,
                    fieldWidth: 40,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
