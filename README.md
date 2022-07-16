<!-- ![workflow](https://github.com/Lodestone-Team/client/actions/workflows/client.yml/badge.svg) -->

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
<p> Thanks to
<a href="https://github.com/Arcslogger" >Wilbur Zhang (Arclogger)  </a>
for the logo!
</p>
  <p align="center">
    Simple, powerful, modular game server hosting tool
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



<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#installation">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#roadmap">Roadmap</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>



<!-- ABOUT THE PROJECT -->
## About Lodestone

**LODESTONE IS STILL IN ITS EARLY PHASE OF DEVELOPMENT, FEATURES ARE SUBJECT TO CHANGE AND NO STABLITY GUARANTEE CAN BE MADE AT THIS TIME**

**USE FOR DEVELOPMENT PURPOSE ONLY!**

<!-- [![Product Name Screen Shot][product-screenshot]](https://example.com) -->

Lodestone is a server wrapper tool (that means it wraps around your server program) that aims to simplfy the process of hosting a game server.

It handles the complicated parts of setting up a server, such as getting dependencies, configuration and port forward. While exposing a simple yet powerful interface to the end user, giving you a truly one-click experience

### Simple

Lodestone is simple to use if you just a server to play with your friends, just download the exectuable and run it.

### Powerful

Lodestone allows you to configure your server to your liking.

It also provides powerful management features to manage your server, such as task scheduling, listening in on events, multi user permissions and authentication, and more.

For the most advanced user, Lodestone allows you to programmatically control your server with Lua.

### Modular

Lodestone is designed from the ground up to be modular. To add support for a new game, simply implements the set of interfaces that Lodestone provides.

<p align="right">(<a href="#top">back to top</a>)</p>



### Built With


* [![Rust][Rust]][Rust-url]
* [![React][React.js]][React-url]
* [![TypeScript][TypeScript]][TypeScript-url]
<!-- * [![Node.js][Node.js]][Node.js-url] -->
<!-- * [![Express][Express]][Express-url] -->

<p align="right">(<a href="#top">back to top</a>)</p>

## Getting Started (end user)

TODO!

<!-- GETTING STARTED -->
## Getting Started (development)

Please make sure you have `cargo` and `rustc`, for instruction on how to install the Rust toolchain, see here: [rustup.rs](https://rustup.rs/).

### Prerequisites

* `build-essential` is required for the Rust toolchain.
  ```sh
  sudo apt-get install build-essential
  ```
* `pkg-config` and `libssl-dev` are required to compile rocket
  ```sh
  sudo apt-get install pkg-config libssl-dev
  ```
* `cpuidtool` is required to query CPU information
  ```sh
  sudo apt-get install cpuidtool libcpuid14 libcpuid-dev
  ```

### Installation


1. Clone the repo
   ```sh
   git clone https://github.com/Lodestone-Team/client
   ```
2. Define the `LODESTONE_PATH` environment variable, this is where Lodestone will store its data for development and testing
   ```sh
   export LODESTONE_PATH={}
   ```
3. Running
   ```sh
   cargo run --main
   ```

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- USAGE EXAMPLES -->
## Usage

TODO!

_For more examples, please refer to the [Documentation](https://example.com)_

<p align="right">(<a href="#top">back to top</a>)</p>



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

Contribution is welcome, please join our [Discord](https://discord.gg/Hpn2dxV9dD) for more details.

TODO!

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- LICENSE -->
## License

TODO!

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- CONTACT -->
<!-- ## Contact

Your Name - [@twitter_handle](https://twitter.com/twitter_handle) - email@email_client.com

Project Link: [https://github.com/github_username/repo_name](https://github.com/github_username/repo_name)

<p align="right">(<a href="#top">back to top</a>)</p> -->



<!-- ACKNOWLEDGMENTS -->
## Credits

Active members of the Lodestone team:

* [Peter Jiang (CheatCod)](https://github.com/CheatCod) - Lead Developer
* [Kevin Huang (Ynng)](https://github.com/Ynng) - Frontend Lead
* [Mark Sun (Lemonsity)](https://github.com/Lemonsity) - Developer
* [Wilbur Zhang (Arclogger)](https://github.com/Arcslogger) - UI Designer, Logo Designer

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
[workflow-shield]: https://img.shields.io/github/workflow/status/Lodestone-Team/client/client?style=for-the-badge
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
