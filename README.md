[![Lodestone Screen Shot](/dashboard/public/assets/readmeRender.png)](https://www.lodestone.cc/)

[![Contributors][contributors-shield]][contributors-url]
[![Issues][issues-shield]][issues-url]
[![Build][workflow-shield]][workflow-url]


# Lodestone

A free, open source server hosting tool for Minecraft and other multiplayers

A free, open source server hosting tool for Minecraft and other multiplayers

🔗 Get Started: [https://www.lodestone.cc/](https://www.lodestone.cc/)

## Features and roadmap

- [x] Clean and intuitive UI
- [x] One-click installation and setup
- [x] Collaborative remote server and resource management
- [x] Priority on safety and security
- [ ] User permission management 🚧
- [ ] Automated macros and tasks (beta, read more [here](https://github.com/Lodestone-Team/lodestone/wiki/Macro-and-Task))
- [ ] Connecting without port forward 🚧


## Installation

### Windows
You can download Lodestone Desktop for Windows from the [releases page](https://github.com/Lodestone-Team/lodestone/releases)

### Docker
See [here](https://github.com/Lodestone-Team/lodestone/wiki/Docker-Support)

### Linux
Download and run [Lodestone CLI](https://github.com/Lodestone-Team/lodestone_cli).

If you would like to use the dashboard:
1. Use a chromium based browser (Chrome, Edge, Brave, etc.) and go to [https://www.lodestone.cc/](https://www.lodestone.cc/)
2. Follow this [guide](https://experienceleague.adobe.com/docs/target/using/experiences/vec/troubleshoot-composer/mixed-content.html?lang=en) to enable mixed content for the site.
3. If you have browser extensions such as HTTPS Everywhere, disable them for the site.

To see why step 2 and 3 are necessary and some possible solutions, see [here](https://github.com/Lodestone-Team/lodestone/wiki/FAQ#why-do-i-need-to-enable-mixedinsecure-content-and-disable-https-is-this-safe)

> **Note**
> Lodestone Desktop for Linux is highly experimental and untested. We won't be able to provide support if you decide to use it.

### MacOS (Intel)
Download and run [Lodestone CLI](https://github.com/Lodestone-Team/lodestone_cli).

If you would like to use the dashboard:
1. Use a chromium based browser (Chrome, Edge, Brave, etc.) and go to [https://www.lodestone.cc/](https://www.lodestone.cc/)
2. Follow this [guide](https://experienceleague.adobe.com/docs/target/using/experiences/vec/troubleshoot-composer/mixed-content.html?lang=en) to enable mixed content for the site.
3. If you have browser extensions such as HTTPS Everywhere, disable them for the site.

To see why step 2 and 3 are necessary and some possible solutions, see [here](https://github.com/Lodestone-Team/lodestone/wiki/FAQ#why-do-i-need-to-enable-mixedinsecure-content-and-disable-https-is-this-safe)

> **Note**
> ARM Macs are not supported yet. See [this issue](https://github.com/Lodestone-Team/lodestone_core/issues/160) for more info.

> **Note**
> Lodestone Desktop for MacOs is highly experimental and untested. We won't be able to provide support if you decide to use it.


## Safety & Security

Lodestone Core is written entirely in safe Rust, and uses`#![forbid(unsafe_code)]`. However **we can't guarantee the safety of the crates and binaries we link to**, as those may use unsafe rust.

Lodestone is created with security as a top priority. While most of the safety critical code such as login and permissions management have been tested thoroughly, **no formal security audit has been done for any part of Lodestone.**


## Contributing

Lodestone is still new and we have *a lot* of features planned for this year. Either way, we'd love to hear your feedback! If you have any suggestions, leave a GitHub issue or talk to us on our [Discord](https://discord.gg/PkHXRQXkf6).


## License

This project uses the GNU Affero General Public License v3.0. See our `LICENSE` file for details. For an alternate (proprietary) license, please raise an issue.


## Team

Lodestone wouldn't be here if it weren't for our contributors. Check out our [team](https://github.com/orgs/Lodestone-Team/people) here!

## Support Us

We strive to make Lodestone open source and free for everyone to use. If you like what we are making please consider [buying us a coffee](https://ko-fi.com/lodestone_team) to support our development.

## Have questions?

Checkout our FAQ here for more info: https://github.com/Lodestone-Team/lodestone/wiki/FAQ

You can also join our [Discord](https://discord.gg/PkHXRQXkf6) to ask questions and help for setting up Lodestone.


<p align="right">(<a href="#top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[contributors-shield]: https://img.shields.io/github/contributors/Lodestone-Team/dashboard?style=for-the-badge
[contributors-url]: https://github.com/Lodestone-Team/dashboard/graphs/contributors

<!-- [forks-shield]: https://img.shields.io/github/forks/github_username/repo_name.svg?style=for-the-badge
[forks-url]: https://github.com/github_username/repo_name/network/members
[stars-shield]: https://img.shields.io/github/stars/github_username/repo_name.svg?style=for-the-badge
[stars-url]: https://github.com/github_username/repo_name/stargazers -->

[issues-shield]: https://img.shields.io/github/issues/Lodestone-Team/dashboard?style=for-the-badge
[issues-url]: https://github.com/Lodestone-Team/dashboard/issues
[workflow-shield]: https://img.shields.io/github/actions/workflow/status/Lodestone-Team/dashboard/desktop.yml?style=for-the-badge
[workflow-url]: https://github.com/Lodestone-Team/dashboard/actions
[license-shield]: https://img.shields.io/github/license/github_username/repo_name.svg?style=for-the-badge
[license-url]: https://github.com/github_username/repo_name/blob/master/LICENSE.txt
[product-screenshot]: images/screenshot.png
