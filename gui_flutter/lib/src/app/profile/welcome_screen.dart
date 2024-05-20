import 'package:flutter/material.dart';
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
                        // TODO
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
