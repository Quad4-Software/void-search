# SPDX-License-Identifier: AGPL-3.0-or-later
"""Installer for SearXNG package."""

from pathlib import Path

from setuptools import find_packages, setup

from searx import get_setting
from searx.version import GIT_URL, VERSION_TAG

with Path('README.rst').open(encoding='utf-8') as f:
    long_description = f.read()

with Path('requirements.txt').open() as f:
    requirements = [line.strip() for line in f.readlines()]

with Path('requirements-dev.txt').open() as f:
    dev_requirements = [line.strip() for line in f.readlines()]

setup(
    name='searxng',
    description="SearXNG is a metasearch engine. Users are neither tracked nor profiled.",
    long_description=long_description,
    license="AGPL-3.0-or-later",
    author='SearXNG',
    author_email='contact@searxng.org',
    python_requires=">=3.10",
    version=VERSION_TAG,
    keywords='metasearch searchengine search web http',
    url=get_setting('brand.docs_url'),
    classifiers=[
        "Development Status :: 5 - Production/Stable",
        "Topic :: Internet",
        "Topic :: Internet :: WWW/HTTP :: HTTP Servers",
        "Topic :: Internet :: WWW/HTTP :: WSGI :: Application",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Python :: 3.13",
    ],
    project_urls={"Code": GIT_URL, "Issue tracker": get_setting('brand.issue_url')},
    entry_points={'console_scripts': ['searxng-run = searx.webapp:run']},
    packages=find_packages(
        include=[
            'searx',
            'searx.*',
            'searx.*.*',
            'searx.*.*.*',
        ]
    ),
    package_data={
        'searx': [
            'settings.yml',
            '*.toml',
            '*.msg',
            'data/*.json',
            'data/*.txt',
            'data/*.ftz',
            'favicons/*.toml',
            'infopage/**',
            'static/**',
            'templates/**',
            'translations/**',
        ],
    },
    install_requires=requirements,
    extras_require={'test': dev_requirements},
)
