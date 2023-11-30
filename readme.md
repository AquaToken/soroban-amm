<div id="top"></div>


<!-- PROJECT SHIELDS -->
[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![MIT License][license-shield]][license-url]



<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/AquaToken/soroban-amm">
    <img src="https://aqua.network/assets/img/header-logo.svg" alt="Logo" width="250" height="80">
  </a>

<h3 align="center">Aquarius</h3>

  <p align="center">
    Aquarius protocol is governed by DAO voting with AQUA tokens. Vote and participate in discussions to shape the future of Aquarius.
    <br />
    <br />
    <a href="https://github.com/AquaToken/soroban-amm/issues">Report Bug</a>
    ·
    <a href="https://gov.aqua.network/">Request Feature</a>
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
        <li><a href="#development-setup">Development setup</a></li>
      </ul>
    </li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#contact">Contact</a></li>
  </ol>
</details>



<!-- ABOUT THE PROJECT -->
## About The Project

[![Aquarius Screen Shot][product-screenshot]](https://aqua.network/)


#### What is Aquarius?
Aquarius is a liquidity layer built on top of the Stellar network. Using a governance system powered by the AQUA token, holders can vote on proposals to change how the Aquarius protocol functions and vote for which Stellar DEX & AMM markets are incentivized with AQUA rewards. Top-voted markets see their liquidity providers receive hourly rewards paid in AQUA, creating a way to earn extra rewards when supporting markets on Stellar.

#### What's the Soroban?
Soroban is a smart contracts platform designed to be sensible, built-to-scale, batteries-included, and developer-friendly.

#### Soroban-powered AMMs
We plan to use Soroban to build Automated Market Maker (AMM) smart contracts and the distribution engine for AQUA liquidity rewards. AQUA rewards will incentivize users to provide liquidity to the Soroban AMM. The distribution engine will run as a set of Soroban smart contracts and will calculate and distribute liquidity rewards to LPs accordingly.

#### Smart Contracts
- **liquidity_pool** - Exchange liquidity pool based on constant product formula (xy=k)
- **liquidity_pool_stableswap** - Exchange liquidity pool designed for extremely efficient stablecoin trading and low risk, supplemental fee income for liquidity providers, without an opportunity cost. It allows users to trade between correlated cryptocurrencies with a bespoke low slippage, low fee algorithm.
- **token** - [SEP-0041](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md) compatible token smart contract designed for liquidity pool share management
- **liquidity_pool_router** - entry point and catalogue of liquidity pools which is capable to deploy new pools if necessary

[![Smart Contracts diagram][contracts-diagram]](https://aqua.network/)

<p align="right">(<a href="#top">back to top</a>)</p>



### Built With

* [Rust](https://www.rust-lang.org/)
* [Soroban](https://soroban.stellar.org/)
* [Rust Soroban SDK](https://github.com/stellar/rs-soroban-sdk)

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- GETTING STARTED -->

## Getting Started

### Prerequisites
- [Task](https://taskfile.dev/) as task runner
- installed latest Rust version
- [soroban cli](https://github.com/stellar/soroban-tools)

### Development setup
Project built using django framework, so setup is similar to generic django project.

#### Clone project
`git clone git@github.com:AquaToken/soroban-amm.git`

#### Build contracts
`task build`

#### Run tests
`task test`

#### (Optionally) Deploy & invoke contracts via soroban-cli
check the Soroban documentation: https://soroban.stellar.org/docs/reference/rpc


<p align="right">(<a href="#top">back to top</a>)</p>


<!-- CONTRIBUTING -->
## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

If you have a suggestion that would make this better, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".
Don't forget to give the project a star! Thanks again!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- CONTACT -->
## Contact

Email: [hello@aqua.network](mailto:hello@aqua.network)
Telegram chat: [@aquarius_HOME](https://t.me/aquarius_HOME)
Telegram news: [@aqua_token](https://t.me/aqua_token)
Twitter: [@aqua_token](https://twitter.com/aqua_token)
GitHub: [@AquaToken](https://github.com/AquaToken)
Discord: [@Aquarius](https://discord.gg/sgzFscHp4C)
Reddit: [@AquariusAqua](https://www.reddit.com/r/AquariusAqua/)
Medium: [@aquarius-aqua](https://medium.com/aquarius-aqua)

Project Link: [https://github.com/AquaToken/soroban-amm](https://github.com/AquaToken/soroban-amm)

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[contributors-shield]: https://img.shields.io/github/contributors/AquaToken/soroban-amm.svg?style=for-the-badge
[contributors-url]: https://github.com/AquaToken/soroban-amm/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/AquaToken/soroban-amm.svg?style=for-the-badge
[forks-url]: https://github.com/AquaToken/soroban-amm/network/members
[stars-shield]: https://img.shields.io/github/stars/AquaToken/soroban-amm.svg?style=for-the-badge
[stars-url]: https://github.com/AquaToken/soroban-amm/stargazers
[issues-shield]: https://img.shields.io/github/issues/AquaToken/soroban-amm.svg?style=for-the-badge
[issues-url]: https://github.com/AquaToken/soroban-amm/issues
[license-shield]: https://img.shields.io/github/license/AquaToken/soroban-amm.svg?style=for-the-badge
[license-url]: https://github.com/AquaToken/soroban-amm/blob/master/LICENSE.txt
[product-screenshot]: images/screenshot_swap.png
[contracts-diagram]: images/diagram.png
