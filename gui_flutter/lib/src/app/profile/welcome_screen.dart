import 'package:flutter/material.dart';
import 'package:gui_flutter/src/app/main_menu.dart';
import 'package:gui_flutter/src/app/profile/new_profile.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class ProfileSelector extends StatefulWidget {
  const ProfileSelector({super.key});

  @override
  State<ProfileSelector> createState() => _ProfileSelectorState();
}

class _ProfileSelectorState extends State<ProfileSelector> {
  late Future<List<String>> _profiles;
  // List<DropdownMenuEntry<String>> _profiles = [];
  String? _selectedProfile;

  @override
  void initState() {
    super.initState();
    updateProfiles();
    // listProfiles().then((value) => _profiles =
    //     value.map((e) => DropdownMenuEntry(value: e, label: e)).toList());
  }

  void updateProfiles() {
    _profiles = Client.getProfiles();
    _profiles.then((value) => {
          if (value.isEmpty)
            {
              Navigator.pushAndRemoveUntil(
                context,
                MaterialPageRoute(builder: (context) => const ServerDialog()),
                (Route<dynamic> route) => false,
              )
            }
        });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Select Profile")),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 50, vertical: 20),
          child: FutureBuilder(
            future: _profiles,
            builder: (context, snapshot) {
              if (snapshot.hasData) {
                var profiles = snapshot.data!;
                return Column(
                  children: [
                    DropdownMenu<String>(
                      expandedInsets: const EdgeInsets.symmetric(horizontal: 8),
                      label: const Text("Profile"),
                      dropdownMenuEntries: profiles
                          .map((e) => DropdownMenuEntry(
                                label: e,
                                value: e,
                              ))
                          .toList(),
                      onSelected: (value) =>
                          setState(() => _selectedProfile = value),
                    ),
                    const SizedBox(height: 20),
                    ElevatedButton(
                      style: ElevatedButton.styleFrom(
                        minimumSize: const Size.fromHeight(50),
                      ),
                      child: const Text("Next"),
                      onPressed: () {
                        if (_selectedProfile != null) {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (context) => UnlockDialog(
                                  selectedProfile: _selectedProfile!),
                            ),
                          );
                        }
                      },
                    ),
                    const SizedBox(height: 20),
                    ElevatedButton.icon(
                      style: ElevatedButton.styleFrom(
                          minimumSize: const Size.fromHeight(50),
                          backgroundColor:
                              const Color.fromARGB(255, 255, 48, 48),
                          foregroundColor: const Color.fromARGB(255, 64, 0, 0)),
                      label: const Text("Delete Profile"),
                      icon: const Icon(Icons.delete),
                      onPressed: () {
                        if (_selectedProfile != null) {
                          showAdaptiveDialog(
                            context: context,
                            builder: (context) => AlertDialog.adaptive(
                              content: Text(
                                  "Are you sure you want to delete \"${_selectedProfile!}\""),
                              actions: [
                                IconButton(
                                  icon: const Icon(Icons.check),
                                  onPressed: () async {
                                    await Client.removeProfile(
                                        profileKey: _selectedProfile!);
                                    Navigator.of(context, rootNavigator: true)
                                        .pop();
                                    updateProfiles();
                                  },
                                ),
                                IconButton(
                                  icon: const Icon(Icons.close),
                                  onPressed: () =>
                                      Navigator.of(context, rootNavigator: true)
                                          .pop(),
                                )
                              ],
                            ),
                          );
                        }
                      },
                    ),
                  ],
                );
              } else {
                return const CircularProgressIndicator();
              }
            },
          ),
        ),
      ),
    );
  }
}

class UnlockDialog extends StatefulWidget {
  const UnlockDialog({super.key, required this.selectedProfile});

  final String selectedProfile;

  @override
  State<StatefulWidget> createState() => _UnlockDialogState();
}

class _UnlockDialogState extends State<UnlockDialog> {
  String _password = "";

  void _unlock() {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => UnlockingLoadingDialog(
          selectedProfile: widget.selectedProfile,
          password: _password,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Unlock Profile")),
      body: Center(
          child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 50, vertical: 20),
        child: Column(
          children: [
            TextField(
              obscureText: true,
              autofocus: true,
              decoration: InputDecoration(
                border: const OutlineInputBorder(),
                labelText: "Password for \"${widget.selectedProfile}\"",
              ),
              onChanged: (value) => setState(() => _password = value),
              onSubmitted: (value) => _unlock(),
            ),
            const SizedBox(height: 20),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                minimumSize: const Size.fromHeight(50),
              ),
              onPressed: _unlock,
              child: const Text("Unlock"),
            ),
          ],
        ),
      )),
    );
  }
}

class UnlockingLoadingDialog extends StatelessWidget {
  const UnlockingLoadingDialog(
      {super.key, required this.selectedProfile, required this.password});

  final String selectedProfile;
  final String password;

  @override
  Widget build(BuildContext context) {
    var openingProfile = Client.openProfileString(
        profileKey: selectedProfile, password: password);

    openingProfile.then((client) {
      Navigator.pushAndRemoveUntil(
        context,
        MaterialPageRoute(
          builder: (context) => MainMenu(
            client: client,
          ),
        ),
        (Route<dynamic> route) => false,
      );
    });

    return Scaffold(
      appBar: AppBar(title: const Text("Unlocking Profile...")),
      body: FutureBuilder(
        future: Client.openProfileString(
            profileKey: selectedProfile, password: password),
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text(snapshot.error.toString()));
          } else {
            return const Center(child: CircularProgressIndicator());
          }
        },
      ),
    );
  }
}
