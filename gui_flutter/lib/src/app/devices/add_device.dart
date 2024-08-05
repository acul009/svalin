import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:pin_code_fields/pin_code_fields.dart';

class AddDeviceDialog extends StatelessWidget {
  const AddDeviceDialog({super.key, required this.client});

  final Client client;

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
                  onCompleted: (value) {
                    var waiting = client.addAgentWithCode(joinCode: value);
                    Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) =>
                              ConfirmDeviceDialog(waiting: waiting),
                        ));
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class ConfirmDeviceDialog extends StatefulWidget {
  const ConfirmDeviceDialog({super.key, required this.waiting});

  final Future<WaitingForConfirmCode> waiting;

  @override
  State<StatefulWidget> createState() {
    return ConfirmDeviceState();
  }
}

class ConfirmDeviceState extends State<ConfirmDeviceDialog> {
  String confirmCode = "";
  String agentName = "";

  final _formKey = GlobalKey<FormState>();

  Future<void> _add(WaitingForConfirmCode waiting) async {
    if (_formKey.currentState!.validate()) {
      Navigator.pushReplacement(
          context,
          MaterialPageRoute(
            builder: (context) => WaitForAgentInitComplete(
                waiting: waiting.confirm(
                    confirmCode: confirmCode, agentName: agentName)),
          ));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Add device")),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(50),
          child: FutureBuilder(
              future: widget.waiting,
              builder: (context, snapshot) {
                if (snapshot.error != null) {
                  return ErrorWidget(snapshot.error!);
                } else if (snapshot.data != null) {
                  return Form(
                    key: _formKey,
                    child: Column(
                      children: [
                        const Text("Connected to agent"),
                        const Text("Enter confirm code"),
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
                            onChanged: (value) =>
                                setState(() => confirmCode = value),
                            validator: (value) {
                              if (confirmCode.length < 6) {
                                return "Required";
                              }

                              return null;
                            },
                          ),
                        ),
                        TextFormField(
                          decoration: const InputDecoration(
                            border: OutlineInputBorder(),
                            labelText: "Name the new agent",
                          ),
                          onChanged: (value) =>
                              setState(() => agentName = value),
                          onFieldSubmitted: (value) => _add(snapshot.data!),
                          validator: (value) {
                            if (value == null || value.isEmpty) {
                              return "Required";
                            }

                            return null;
                          },
                        ),
                        ElevatedButton(
                          onPressed: () => _add(snapshot.data!),
                          child: const Text("Add agent"),
                        )
                      ],
                    ),
                  );
                } else {
                  return const Column(
                    children: [
                      Text("Connecting to agent..."),
                      CircularProgressIndicator(),
                    ],
                  );
                }
              }),
        ),
      ),
    );
  }
}

class WaitForAgentInitComplete extends StatelessWidget {
  const WaitForAgentInitComplete({super.key, required this.waiting});

  final Future<void> waiting;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Add device")),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(50),
          child: FutureBuilder(
            future: waiting,
            builder: (context, snapshot) {
              if (snapshot.connectionState == ConnectionState.done) {
                if (snapshot.hasError) {
                  return ErrorWidget(snapshot.error!);
                } else {
                  Navigator.pop(context);
                  Navigator.pop(context);
                  Navigator.pop(context);
                  return CircularProgressIndicator();
                }
              } else {
                return const Column(
                  children: [
                    Text("initializing agent..."),
                    CircularProgressIndicator(),
                  ],
                );
              }
            },
          ),
        ),
      ),
    );
  }
}
