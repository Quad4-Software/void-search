# -*- mode: python -*-
# SPDX-License-Identifier: AGPL-3.0-or-later
"""A SearXNG message file, see :py:obj:`searx.babel`"""

import typing

from searx import engines, webutils
from searx.weather import WeatherConditionType

__all__ = [
    'BRAND_CUSTOM_LINKS',
    'CATEGORY_NAMES',
    'CONSTANT_NAMES',
    'SOCIAL_MEDIA_TERMS',
    'STYLE_NAMES',
    'WEATHER_CONDITIONS',
    'WEATHER_TERMS',
]

CONSTANT_NAMES = {
    # Constants defined in other modules
    'NO_SUBGROUPING': webutils.NO_SUBGROUPING,
    'DEFAULT_CATEGORY': engines.DEFAULT_CATEGORY,
}

CATEGORY_NAMES = {
    "APPS": "apps",
    "BLOGS": "blogs",
    "DICTIONARIES": "dictionaries",
    "FILES": "files",
    "GENERAL": "general",
    "ICONS": "icons",
    "IMAGES": "images",
    "IT": "it",
    "LYRICS": "lyrics",
    "MAP": "map",
    "MOVIES": "movies",
    "MUSIC": "music",
    "NEWS": "news",
    "ONIONS": "onions",
    "PACKAGES": "packages",
    "Q_A": "q&a",
    "RADIO": "radio",
    "REPOS": "repos",
    "SCIENCE": "science",
    "SCIENTIFIC PUBLICATIONS": "scientific publications",
    "SOCIAL_MEDIA": "social media",
    "SOFTWARE_WIKIS": "software wikis",
    "STOCK_IMAGES": "stock images",
    "TRANSLATE": "translate",
    "TV": "tv",
    "VIDEOS": "videos",
    "WEATHER": "weather",
    "WEB": "web",
}

STYLE_NAMES = {
    'AUTO': 'auto',
    'LIGHT': 'light',
    'DARK': 'dark',
    'BLACK': 'black',
}

BRAND_CUSTOM_LINKS = {
    'UPTIME': 'Uptime',
    'ABOUT': 'About',
}

WEATHER_TERMS = {
    'AVERAGE TEMP.': 'Average temp.',
    'CLOUD COVER': 'Cloud cover',
    'CONDITION': 'Condition',
    'CURRENT CONDITION': 'Current condition',
    'EVENING': 'Evening',
    'FEELS LIKE': 'Feels like',
    'HUMIDITY': 'Humidity',
    'MAX TEMP.': 'Max temp.',
    'MIN TEMP.': 'Min temp.',
    'MORNING': 'Morning',
    'NIGHT': 'Night',
    'NOON': 'Noon',
    'PRESSURE': 'Pressure',
    'SUNRISE': 'Sunrise',
    'SUNSET': 'Sunset',
    'TEMPERATURE': 'Temperature',
    'UV INDEX': 'UV index',
    'VISIBILITY': 'Visibility',
    'WIND': 'Wind',
}


WEATHER_CONDITIONS = [
    # The capitalized string goes into to i18n/l10n (en: "Clear sky" -> de: "wolkenloser Himmel")
    msg.capitalize()
    for msg in typing.get_args(WeatherConditionType)
]

SOCIAL_MEDIA_TERMS = {
    'SUBSCRIBERS': 'subscribers',
    'POSTS': 'posts',
    'ACTIVE USERS': 'active users',
    'COMMENTS': 'comments',
    'USER': 'user',
    'COMMUNITY': 'community',
    'POINTS': 'points',
    'TITLE': 'title',
    'AUTHOR': 'author',
    'THREAD OPEN': 'open',
    'THREAD CLOSED': 'closed',
    'THREAD ANSWERED': 'answered',
}
