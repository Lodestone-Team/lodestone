
<div id="top"></div>
<!--
*** Thanks for checking out the Best-README-Template. If you have a suggestion
*** that would make this better, please fork the repo and create a pull request
*** or simply open an issue with the tag "enhancement".
*** Don't forget to give the project a star!
*** Thanks again! Now go create something AMAZING! :D
-->



<!-- PROJECT SHIELDS -->
<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->
[![Contributors][contributors-shield]][contributors-url]
[![Issues][issues-shield]][issues-url]
[![Build][workflow-shield]][workflow-url]
<!-- [![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url] -->
<!-- [![MIT License][license-shield]][license-url] -->
<!-- [![LinkedIn][linkedin-shield]][linkedin-url] -->



<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/Lodestone-Team/client">
    <img src="readme/lodestone_logo.svg" alt="Logo" width="80" height="80">
  </a>

<h3 align="center">Lodestone</h3>
  <p align="center">
    Client for the Lodestone project
    <br />
    <a href="https://beta.lodestone.cc/"><strong>Website(Beta WIP) »</strong></a>
    <br />
    <br />
    <!-- <a href="https://github.com/github_username/repo_name">View Demo</a> -->
    ·
    <a href="https://github.com/Lodestone-Team/client/issues">Report Bug</a>
    ·
    <a href="https://github.com/Lodestone-Team/client/issues">Request Feature</a>
  </p>
</div>





<!-- ABOUT THE PROJECT -->





### Built With


* [![Rust][Rust]][Rust-url]
<!-- * [![Node.js][Node.js]][Node.js-url] -->
<!-- * [![Express][Express]][Express-url] -->

<p align="right">(<a href="#top">back to top</a>)</p>

## Getting Started (end user)

Follow the instruction on our [Github page](https://github.com/Lodestone-Team#installation)

### Using Docker Image

As of v0.4.3 we have added Docker support to lodestone core.
Our precompiled images will be based on the newest release available from: `ghcr.io/lodestone-team/lodestone_core`.
Alternatively, you may build your own image using the default `Dockerfile`, not additional arguments required.

> **Note**
> You may add additional ports as you wish to forward, but 16662 is the default port served in the image.
> You may add a volume for your lodestone instance to be accessible, in the example below, you can create a volume first by using `docker volume create lodestone`.

To use:
```sh
docker run -d \
  --name lodestone \
  --restart unless-stopped \
  -p 16662:16662 \
  -v lodestone:/root/.lodestone \
  ghcr.io/Lodestone-Team/lodestone_core
```

<!-- GETTING STARTED -->
## Getting Started (development)

Please make sure you have `cargo` and `rustc`, for instruction on how to install the Rust toolchain, see here: [rustup.rs](https://rustup.rs/).

### Prerequisites

These instructions apply to Ubuntu 20.04 LTS and later.

* `build-essential` is required for the Rust toolchain.
  ```sh
  sudo apt-get install build-essential
  ```
* `pkg-config` and `libssl-dev` are required to compile Axum
  ```sh
  sudo apt-get install pkg-config libssl-dev
  ```
* `cpuidtool` is required to query CPU information
  ```sh
  sudo apt-get install cpuidtool libcpuid-dev
  ```
  A few other packages are needed to compile Lodestone
  ```sh
  sudo apt-get install libffi-dev libmagic-dev file
  ```

### Installation
#### Running the client

1. Clone the repo
   ```sh
   git clone https://github.com/Lodestone-Team/client
   ```
2. By default Lodestone stores its data in `~/.lodestone`. If you would like to override it for development & testing please define the `LODESTONE_PATH` environment variable to override it.
   ```sh
   export LODESTONE_PATH=~/test_dev
   ```
3. Running
   ```sh
   cargo run --bin main
   ```

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- USAGE EXAMPLES -->
<!-- ## Usage

Follow the instruction on our [Github page](https://github.com/Lodestone-Team#installation)

_For more examples, please refer to the [Documentation](https://example.com)_

<p align="right">(<a href="#top">back to top</a>)</p> -->



<!-- ROADMAP -->
<!-- ## Roadmap

- [ ] Feature 1
- [ ] Feature 2
- [ ] Feature 3
    - [ ] Nested Feature

See the [open issues](https://github.com/github_username/repo_name/issues) for a full list of proposed features (and known issues).

<p align="right">(<a href="#top">back to top</a>)</p> -->



<!-- CONTRIBUTING -->
## Contributing

Contribution is welcome, please join our [Discord](https://discord.gg/yKrSZXbhNx) for more details.

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- LICENSE -->
## License

This project uses the GNU Affero General Public License v3.0. See our LICENSE files for details. For an alternate (commercial) license, please raise an issue.

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- CONTACT -->
<!-- ## Contact

Your Name - [@twitter_handle](https://twitter.com/twitter_handle) - email@email_client.com

Project Link: [https://github.com/github_username/repo_name](https://github.com/github_username/repo_name)

<p align="right">(<a href="#top">back to top</a>)</p> -->



<!-- ACKNOWLEDGMENTS -->
## Credits

Active members of the Lodestone client team:

* [Peter Jiang (CheatCod)](https://github.com/CheatCod) - Lead Developer
* [Kevin Huang (Ynng)](https://github.com/Ynng) - Developer
* [Mark Sun (Lemonsity)](https://github.com/Lemonsity) - Developer

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[contributors-shield]: https://img.shields.io/github/contributors/Lodestone-Team/client?style=for-the-badge
[contributors-url]: https://github.com/Lodestone-Team/client/graphs/contributors
<!-- [forks-shield]: https://img.shields.io/github/forks/github_username/repo_name.svg?style=for-the-badge
[forks-url]: https://github.com/github_username/repo_name/network/members
[stars-shield]: https://img.shields.io/github/stars/github_username/repo_name.svg?style=for-the-badge
[stars-url]: https://github.com/github_username/repo_name/stargazers -->
[issues-shield]: https://img.shields.io/github/issues/Lodestone-Team/client?style=for-the-badge
[issues-url]: https://github.com/Lodestone-Team/client/issues
[workflow-shield]: https://img.shields.io/github/actions/workflow/status/Lodestone-Team/client/client.yml?style=for-the-badge
[workflow-url]: https://github.com/Lodestone-Team/client/actions
[license-shield]: https://img.shields.io/github/license/github_username/repo_name.svg?style=for-the-badge
[license-url]: https://github.com/github_username/repo_name/blob/master/LICENSE.txt
[product-screenshot]: images/screenshot.png
[React.js]: https://img.shields.io/badge/React-20232A?style=for-the-badge&logo=react&logoColor=61DAFB
[React-url]: https://reactjs.org/
[Rust]: https://img.shields.io/badge/RUST-000000?style=for-the-badge&logo=RUST&logoColor=white
[Rust-url]: https://www.rust-lang.org/
[TypeScript]: https://img.shields.io/badge/TypeScript-000000?style=for-the-badge&logo=TypeScript&logoColor=white
[TypeScript-url]: https://www.typescriptlang.org/
[Node.js]: https://img.shields.io/badge/Node.js-000000?style=for-the-badge&logo=Node.js&logoColor=white
[Node.js-url]: https://nodejs.org/en/
[Express.js]: https://img.shields.io/badge/Express.js-000000?style=for-the-badge&logo=Express.js&logoColor=white
[Express.js-url]: https://expressjs.com/
<!-- [linkedin-shield]: https://img.shields.io/badge/-LinkedIn-black.svg?style=for-the-badge&logo=linkedin&colorB=555
[linkedin-url]: https://linkedin.com/in/linkedin_username -->
<!-- [Next.js]: https://img.shields.io/badge/next.js-000000?style=for-the-badge&logo=nextdotjs&logoColor=white
[Next-url]: https://nextjs.org/ -->
<!-- [Vue.js]: https://img.shields.io/badge/Vue.js-35495E?style=for-the-badge&logo=vuedotjs&logoColor=4FC08D
[Vue-url]: https://vuejs.org/
[Angular.io]: https://img.shields.io/badge/Angular-DD0031?style=for-the-badge&logo=angular&logoColor=white
[Angular-url]: https://angular.io/
[Svelte.dev]: https://img.shields.io/badge/Svelte-4A4A55?style=for-the-badge&logo=svelte&logoColor=FF3E00
[Svelte-url]: https://svelte.dev/
[Laravel.com]: https://img.shields.io/badge/Laravel-FF2D20?style=for-the-badge&logo=laravel&logoColor=white
[Laravel-url]: https://laravel.com
[Bootstrap.com]: https://img.shields.io/badge/Bootstrap-563D7C?style=for-the-badge&logo=bootstrap&logoColor=white
[Bootstrap-url]: https://getbootstrap.com
[JQuery.com]: https://img.shields.io/badge/jQuery-0769AD?style=for-the-badge&logo=jquery&logoColor=white
[JQuery-url]: https://jquery.com  -->

<!-- [Trello](https://trello.com/b/sCaSEPyU/lodestone)
[Figma](https://www.figma.com/file/gM7KUynANg4JkGF3QBsYJ9/Lodestone?node-id=166%3A1621) -->
