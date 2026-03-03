using TWF.Utilities;
using System;

// Test with CJK_CharacterWidth = 2 (default)
CharacterWidthHelper.CJKCharacterWidth = 2;
string testCJK = "日本語";
Console.WriteLine($"CJK_CharacterWidth=2: '{testCJK}' width = {CharacterWidthHelper.GetStringWidth(testCJK)} (expected 6)");

// Test with CJK_CharacterWidth = 0 (disabled)
CharacterWidthHelper.CJKCharacterWidth = 0;
Console.WriteLine($"CJK_CharacterWidth=0: '{testCJK}' width = {CharacterWidthHelper.GetStringWidth(testCJK)} (expected 3)");

// Test with ASCII
string testASCII = "Hello";
Console.WriteLine($"CJK_CharacterWidth=0: '{testASCII}' width = {CharacterWidthHelper.GetStringWidth(testASCII)} (expected 5)");

// Reset to default
CharacterWidthHelper.CJKCharacterWidth = 2;
Console.WriteLine($"CJK_CharacterWidth=2: '{testASCII}' width = {CharacterWidthHelper.GetStringWidth(testASCII)} (expected 5)");
